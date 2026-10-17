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

// TODO: Other check

pub fn check_delta(delta: &EnvDelta) -> Result<(), error::CheckError> {
    check_path_exists(delta)?;
    Ok(())
}

pub mod error {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum CheckError {
        #[error("Not exists: {0}")]
        NotExists(String),
    }
}
