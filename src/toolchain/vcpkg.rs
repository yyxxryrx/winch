use super::VcpkgPlan;
use super::error::{LoadError, LoadResult};
use crate::env::{EnvDelta, EnvObj};
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

fn env(value: impl Into<String>) -> EnvObj {
    EnvObj::new(value.into(), "vcpkg")
}

pub(super) fn plan(config: &Vcpkg) -> LoadResult<VcpkgPlan> {
    Ok(VcpkgPlan {
        root: find_root()?,
        triplet: config.triplet.clone(),
    })
}

pub(super) fn apply(plan: &VcpkgPlan, delta: &mut EnvDelta) -> LoadResult<()> {
    let root = &plan.root;
    debug!("Find vcpkg root successfully ... {}", root.display());
    info!("Load VCPKG");

    let installed = root.join("installed").join(&plan.triplet);
    delta.set("VCPKG_ROOT", env(root.to_string_lossy()));
    delta.set("VCPKG_TARGET_TRIPLET", env(plan.triplet.clone()));
    delta.set(
        "CMAKE_TOOLCHAIN_FILE",
        env(root
            .join("scripts")
            .join("buildsystems")
            .join("vcpkg.cmake")
            .to_string_lossy()),
    );

    delta.prepend("LIB", env(installed.join("lib").to_string_lossy()));
    delta.prepend("Path", env(installed.join("bin").to_string_lossy()));
    delta.prepend("INCLUDE", env(installed.join("include").to_string_lossy()));
    Ok(())
}
