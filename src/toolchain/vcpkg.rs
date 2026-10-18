use super::error::{LoadError, LoadResult};
use crate::env::EnvDelta;
use crate::find;
use crate::project_config::Vcpkg;
use log::{debug, info};
use std::path::PathBuf;

fn find_root() -> LoadResult<PathBuf> {
    find::find_vcpkg().ok_or_else(|| LoadError::CannotFind("vcpkg root".to_string()))
}

pub(crate) fn check(config: &Vcpkg) -> LoadResult<()> {
    let root = find_root()?;
    if !root.join("installed").join(&config.triplet).exists() {
        return Err(LoadError::GetFailed(config.triplet.to_owned()));
    }
    Ok(())
}

pub(super) fn apply(config: &Vcpkg, delta: &mut EnvDelta) -> LoadResult<()> {
    let root = find_root()?;
    debug!("Find vcpkg root successfully ... {}", root.display());
    info!("Load VCPKG");

    let installed = root.join("installed").join(&config.triplet);
    delta.set("VCPKG_ROOT", root.to_string_lossy());
    delta.prepend("LIB", installed.join("lib").to_string_lossy());
    delta.prepend("Path", installed.join("bin").to_string_lossy());
    delta.prepend("INCLUDE", installed.join("include").to_string_lossy());
    Ok(())
}
