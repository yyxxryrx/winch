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

        if let Some(windows_sdk) = config.lib.windows_sdk.as_ref() {
            match crate::toolchain::lib::windows_sdk::check(windows_sdk) {
                Ok(tool) => info!(
                    "windows SDK {} found ... {}",
                    tool.version_text(),
                    tool.path.display()
                ),
                Err(e) => errors.push(format!("windows_sdk: {e}")),
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
        let paths = delta
            .prepend_values("Path")
            .into_iter()
            .chain(delta.append_values("Path"));
        for path in paths {
            for p in path {
                for part in p.value.split(';') {
                    if part.is_empty() {
                        continue;
                    }

                    if !Path::new(part).exists() {
                        return Err(error::CheckError::NotExists(part.to_string()));
                    }
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

#[cfg(test)]
mod tests {
    use super::env_delta::check_delta;
    use crate::env::{EnvDelta, EnvObj};

    #[test]
    fn splits_semicolon_separated_paths() {
        let mut delta = EnvDelta::default();
        let temp = std::env::temp_dir();
        let joined = format!("{};{}", temp.display(), temp.display());
        delta.prepend("Path", EnvObj::new(joined, "test"));

        assert!(check_delta(&delta).is_ok());
    }

    #[test]
    fn finds_path_case_insensitively() {
        let mut delta = EnvDelta::default();
        let temp = std::env::temp_dir();
        delta.prepend("PATH", EnvObj::new(temp.display().to_string(), "test"));

        assert!(check_delta(&delta).is_ok());
    }

    #[test]
    fn detects_missing_path_in_list() {
        let mut delta = EnvDelta::default();
        let temp = std::env::temp_dir();
        let missing = temp.join(format!(
            "winch-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let joined = format!("{};{}", temp.display(), missing.display());
        delta.append("Path", EnvObj::new(joined, "test"));

        assert!(check_delta(&delta).is_err());
    }
}
