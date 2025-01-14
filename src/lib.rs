use std::fmt;
use serde_derive::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::borrow::Cow;

pub mod postinst;

fn create_true() -> bool {
    true
}

pub trait PackageConfig {
    fn config(&self) -> &HashMap<String, Config>;
}

impl<'a, T> PackageConfig for &'a T where T: PackageConfig {
    fn config(&self) -> &HashMap<String, Config> {
        (*self).config()
    }
}

impl<'a> PackageConfig for PackageInstance<'a> {
    fn config(&self) -> &HashMap<String, Config> {
        &self.spec.config()
    }
}

impl<'a> PackageConfig for ServiceInstance<'a> {
    fn config(&self) -> &HashMap<String, Config> {
        &self.spec.config()
    }
}

impl PackageConfig for ServicePackageSpec {
    fn config(&self) -> &HashMap<String, Config> {
        &self.config
    }
}

#[derive(Deserialize)]
pub struct Package {
    pub name: String,
    #[serde(default)]
    pub variants: HashSet<String>,
    #[serde(flatten)]
    pub spec: PackageSpec,
}

fn load_include(dir: &Path, name: &str) -> Package {
    let mut file = dir.join(name);
    file.set_extension("sps");
    Package::load(file)
}

pub fn load_file<T: for<'a> serde::Deserialize<'a>, P: AsRef<Path>>(file: P) -> T {
    let file = file.as_ref();
    let spec = std::fs::read(file).unwrap_or_else(|err| panic!("Failed to read {}: {}", file.display(), err));
    toml::from_slice(&spec).unwrap_or_else(|err| panic!("Failed to parse {}: {}", file.display(), err))
}

impl Package {
    pub fn load<P: AsRef<Path>>(file: P) -> Self {
        load_file(file)
    }

    pub fn load_includes<P: AsRef<Path>>(&self, dir: P) -> HashMap<String, Package> {
        let mut result = HashMap::new();
        for (_, conf) in self.config() {
            if let ConfType::Dynamic { evars, .. } = &conf.conf_type {
                for (pkg, _) in evars {
                    result.entry(pkg.to_owned()).or_insert_with(|| load_include(dir.as_ref(), pkg));
                }
            }
        }

        if let PackageSpec::ConfExt(confext) = &self.spec {
            result.entry(confext.extends.clone()).or_insert_with(|| load_include(dir.as_ref(), &confext.extends));
        }

        result
    }

    pub fn instantiate<'a>(&'a self, variant: Option<&'a str>, includes: Option<&'a HashMap<String, Package>>) -> Option<PackageInstance<'a>> {
        let name = if let Some(variant) = variant {
            // Sanity check
            if !self.variants.contains(variant) {
                return None;
            }

            (&[&self.name.as_str(), variant]).join("-").into()
        } else {
            if self.variants.len() > 0 {
                return None;
            }
            (&self.name).into()
        };

        Some(PackageInstance {
            name,
            variant,
            spec: &self.spec,
            includes,
        })
    }
}

impl PackageConfig for Package {
    fn config(&self) -> &HashMap<String, Config> {
        self.spec.config()
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum PackageSpec {
    Service(ServicePackageSpec),
    ConfExt(ConfExtPackageSpec),
}

impl PackageSpec {
    pub fn summary(&self) -> &Option<String> {
        match self {
            PackageSpec::Service(service) => &service.summary,
            PackageSpec::ConfExt(confext) => &confext.summary,
        }
    }

    pub fn long_doc(&self) -> &Option<String> {
        match self {
            PackageSpec::Service(service) => &service.long_doc,
            PackageSpec::ConfExt(confext) => &confext.long_doc,
        }
    }
}

impl PackageConfig for PackageSpec {
    fn config(&self) -> &HashMap<String, Config> {
        match self {
            PackageSpec::Service(service) => &service.config,
            PackageSpec::ConfExt(confext) => &confext.config,
        }
    }
}

#[derive(Deserialize)]
pub struct ServicePackageSpec {
    pub bin_package: String,
    pub binary: String,
    #[serde(default)]
    pub conf_param: Option<String>,
    #[serde(default)]
    pub conf_d: Option<ConfDir>,
    pub user: UserSpec,
    #[serde(default)]
    pub config: HashMap<String, Config>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default)]
    pub extra_service_config: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub long_doc: Option<String>,
}

#[derive(Deserialize)]
pub struct ConfExtPackageSpec {
    pub extends: String,
    #[serde(default)]
    pub replaces: bool,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub long_doc: Option<String>,
    #[serde(default)]
    pub config: HashMap<String, Config>,
}

#[derive(Deserialize)]
pub struct ConfDir {
    pub param: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct UserSpec {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub group: bool,
    #[serde(default)]
    pub create: Option<CreateUser>,
}

#[derive(Deserialize)]
pub struct CreateUser {
    pub home: bool,
}

#[derive(Deserialize)]
pub struct Config {
    #[serde(default)]
    pub public: bool,
    #[serde(flatten)]
    pub conf_type: ConfType,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum ConfType {
    Static { content: String, #[serde(default)] internal: bool, },
    Dynamic {
        format: ConfFormat,
        #[serde(default)]
        ivars: HashMap<String, InternalVar>,
        #[serde(default)]
        evars: HashMap<String, HashMap<String, ExternalVar>>,
        #[serde(default)]
        hvars: HashMap<String, HiddenVar>,
        cat_dir: Option<String>,
        #[serde(default)]
        cat_files: HashSet<String>,
        comment: Option<String>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfFormat {
    Plain,
    Toml,
}

impl fmt::Display for ConfFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfFormat::Plain => write!(f, "plain"),
            ConfFormat::Toml => write!(f, "toml"),
        }
    }
}


#[derive(Deserialize)]
pub struct InternalVar {
    #[serde(flatten)]
    pub ty: VarType,
    pub summary: String,
    #[serde(default)]
    pub long_doc: Option<String>,
    #[serde(default)]
    pub default: Option<String>,
    pub priority: DebconfPriority,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebconfPriority {
    Low,
    Medium,
    High,
    Critical,
    Dynamic { script: String },
}

#[derive(Deserialize)]
pub struct ExternalVar {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "create_true")]
    pub store: bool,
}

#[derive(Deserialize)]
pub struct HiddenVar {
    #[serde(flatten)]
    pub ty: VarType,
    #[serde(flatten)]
    pub val: HiddenVarVal,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HiddenVarVal {
    Constant(String),
    Script(String),
}

#[derive(Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum VarType {
    String,
    Uint,
    Bool,
    BindHost,
    BindPort,
    Path { file_type: Option<FileType>, create: Option<CreateFsObj>, },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileType {
    Regular,
    Dir,
}

#[derive(Deserialize)]
pub struct CreateFsObj {
    // TODO: use better type
    pub mode: u16,
    pub owner: String,
    pub group: String,
}

pub struct PackageInstance<'a> {
    pub name: Cow<'a, str>,
    pub variant: Option<&'a str>,
    pub spec: &'a PackageSpec,
    pub includes: Option<&'a HashMap<String, Package>>,
}

impl<'a> PackageInstance<'a> {
    pub fn as_service<'b>(&'b self) -> Option<ServiceInstance<'b>> {
        if let PackageSpec::Service(service) = &self.spec {
            Some(ServiceInstance {
                name: &self.name,
                variant: self.variant,
                spec: service,
                includes: self.includes,
            })
        } else {
            None
        }
    }
}

pub struct ServiceInstance<'a> {
    pub name: &'a Cow<'a, str>,
    pub variant: Option<&'a str>,
    pub spec: &'a ServicePackageSpec,
    pub includes: Option<&'a HashMap<String, Package>>,
}

impl<'a> ServiceInstance<'a> {
    pub fn user_name(&self) -> &'a str {
        self.spec.user.name.as_ref().map(AsRef::as_ref).unwrap_or(&self.name.as_ref())
    }

    pub fn service_name(&self) -> &'a str {
        &**self.name
    }

    pub fn service_group(&self) -> Option<&'a str> {
        if self.spec.user.group {
            Some(self.spec.user.name.as_ref().map(AsRef::as_ref).unwrap_or(&**self.name))
        } else {
            None
        }
    }
}
