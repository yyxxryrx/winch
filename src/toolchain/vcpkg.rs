use super::error::{LoadError, LoadResult};
use crate::env::EnvDelta;
use crate::find;
use crate::project_config::Vcpkg;
use log::{debug, info};

pub(super) fn apply(config: &Vcpkg, delta: &mut EnvDelta) -> LoadResult<()> {
    let root = find::find_vcpkg().ok_or_else(|| LoadError::CannotFind("vcpkg root".to_string()))?;
    debug!("Find vcpkg root successfully ... {}", root.display());
    info!("Load VCPKG");

    let installed = root.join("installed").join(&config.triplet);
    delta.set("VCPKG_ROOT", root.to_string_lossy());
    delta.prepend("LIB", installed.join("lib").to_string_lossy());
    delta.prepend("Path", installed.join("bin").to_string_lossy());
    delta.prepend("INCLUDE", installed.join("include").to_string_lossy());
    Ok(())
}
