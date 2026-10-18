pub mod config {
    use log::info;

    pub fn config_check(config: &crate::project_config::Config) -> Vec<String> {
        let mut errors = Vec::new();

        if let Some(msys2) = config.toolchain.as_ref().and_then(|t| t.msys2.as_ref())
            && msys2.enable
        {
            match crate::toolchain::msys2::check() {
                Ok(msys2) => info!("msys2 root found ... {}", msys2.display()),
                Err(e) => errors.push(format!("msys2: {e}")),
            }
        }

        if let Some(msvc) = config.toolchain.as_ref().and_then(|t| t.msvc.as_ref()) {
            match crate::toolchain::msvc::check(msvc) {
                Ok(vs) => info!("msvc found ... {}", vs.installed_path.display()),
                Err(e) => errors.push(format!("msvc: {e}")),
            }
        }

        if let Some(vcpkg) = config.dev.as_ref().and_then(|d| d.vcpkg.as_ref())
            && vcpkg.enable
        {
            match crate::toolchain::vcpkg::check(vcpkg) {
                Ok(()) => info!("vcpkg found"),
                Err(e) => errors.push(format!("vcpkg: {e}")),
            }
        }

        if let Some(java) = config.toolchain.as_ref().and_then(|t| t.java.as_ref()) {
            match crate::toolchain::java::check(java) {
                Ok(tool) => info!("java found ... {}", tool.path.display()),
                Err(e) => errors.push(format!("java: {e}")),
            }
        }

        errors
    }
}

pub mod env_delta {
    use crate::check::error;
    use crate::env::EnvDelta;
    use std::path::Path;

    fn check_path_exists(delta: &EnvDelta) -> Result<(), error::CheckError> {
        if let Some(path) = delta.prepend.get("Path").map(|p| p.split(';')) {
            for p in path {
                if p.is_empty() {
                    continue;
                }

                if !Path::new(p).exists() {
                    return Err(error::CheckError::NotExists(p.to_string()));
                }
            }
        }
        Ok(())
    }

    pub fn check_delta(delta: &EnvDelta) -> Result<(), error::CheckError> {
        check_path_exists(delta)?;
        Ok(())
    }
}

pub mod error {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum CheckError {
        #[error("Not exists: {0}")]
        NotExists(String),
    }
}
