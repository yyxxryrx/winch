pub(crate) mod error;
pub(crate) mod java;
pub(crate) mod msvc;
pub(crate) mod vcpkg;

use crate::env::EnvDelta;
use crate::project_config::Config;

pub use error::{LoadError, LoadResult, ToolType};

pub fn resolve(config: &Config) -> LoadResult<EnvDelta> {
    let mut delta = EnvDelta::default();

    if let Some(msvc) = config.toolchain.as_ref().and_then(|t| t.msvc.as_ref()) {
        msvc::apply(msvc, &mut delta)?;
    }

    if let Some(vcpkg) = config.dev.as_ref().and_then(|d| d.vcpkg.as_ref())
        && vcpkg.enable
    {
        vcpkg::apply(vcpkg, &mut delta)?;
    }

    if let Some(java) = config.toolchain.as_ref().and_then(|t| t.java.as_ref()) {
        java::apply(java, &mut delta)?;
    }

    Ok(delta)
}
