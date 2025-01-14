use std::io::{self, Write};
use debcrafter::{PackageInstance, PackageConfig, ConfType, VarType};
use crate::codegen::{LazyCreateBuilder};

pub fn generate(instance: &PackageInstance, out: LazyCreateBuilder) -> io::Result<()> {
    let mut out = out.finalize();

    for (_, config) in instance.config() {
        if let ConfType::Dynamic { ivars, .. } = &config.conf_type {
            for (var, var_spec) in ivars {
                out.separator("\n")?;

                writeln!(out, "Template: {}/{}", instance.name, var)?;

                let template_type = if let VarType::Bool = var_spec.ty {
                    "bool"
                } else {
                    "string"
                };
                writeln!(out, "Type: {}", template_type)?;

                if let Some(default) = &var_spec.default {
                    writeln!(out, "Default: {}", default)?;
                }
                writeln!(out, "Description: {}", var_spec.summary)?;
                if let Some(long_doc) = &var_spec.long_doc {
                    crate::codegen::paragraph(&mut out, long_doc)?;
                }
            }
        }
    }
    Ok(())
}
