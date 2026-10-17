use super::error::{LoadError, LoadResult};
use crate::env::EnvDelta;
use crate::find;
use log::{debug, info};

pub(super) fn apply(requirement: &str, delta: &mut EnvDelta) -> LoadResult<()> {
    let version = semver::VersionReq::parse(requirement)?;
    let jdk = find::find_jdk_with_version()?
        .into_iter()
        .find(|tool| version.matches(&tool.version))
        .ok_or_else(|| LoadError::CannotFind("target java".to_string()))?;
    debug!("Select java ... {}", jdk.path.display());

    let bin = jdk
        .path
        .parent()
        .ok_or_else(|| LoadError::GetFailed("java bin".to_string()))?;
    let java_home = bin
        .parent()
        .ok_or_else(|| LoadError::GetFailed("JAVA_HOME".to_string()))?;
    debug!("Get JAVA_HOME ... {}", java_home.display());

    info!("Load java");
    delta.set("JAVA_HOME", java_home.to_string_lossy());
    delta.prepend("Path", bin.to_string_lossy());
    Ok(())
}
