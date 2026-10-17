use thiserror::Error;

use crate::{
    msvc::{GetEnvError, RunError, VersionParseError},
    version::GetVersionError,
};

pub type LoadResult<T> = Result<T, LoadError>;

#[derive(Debug)]
pub enum ToolType {
    VSWhere,
}

impl std::fmt::Display for ToolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VSWhere => write!(f, "vswhere"),
        }
    }
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("Cannot find {0} tool")]
    CannotFindTool(ToolType),
    #[error("Cannot find {0}")]
    CannotFind(String),
    #[error("Get {0} failed")]
    GetFailed(String),
    #[error("ConvertError: {0}")]
    ConvertError(#[from] VersionParseError),
    #[error("RunError: {0}")]
    RunError(#[from] RunError),
    #[error("GetVersionError: {0}")]
    GetVersionError(#[from] GetVersionError),
    #[error("GetEnvError: {0}")]
    GetEnvError(#[from] GetEnvError),
    #[error("ParseVersionReqError: {0}")]
    ParseVersionReqError(#[from] semver::Error),
}
