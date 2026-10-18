use super::JavaPlan;
use super::error::{LoadError, LoadResult};
use crate::env::EnvDelta;
use crate::find;
use log::{debug, info};

fn find_jdk(requirement: &str) -> LoadResult<find::Tool> {
    let version = semver::VersionReq::parse(requirement)?;
    find::find_jdk_with_version()?
        .into_iter()
        .find(|tool| version.matches(&tool.version))
        .ok_or_else(|| LoadError::CannotFind("target java".to_string()))
}

fn env(value: impl Into<String>) -> crate::env::EnvObj {
    crate::env::EnvObj::new(value.into(), "java")
}

pub(crate) fn check(requirement: &str) -> LoadResult<find::Tool> {
    find_jdk(requirement)
}

pub(super) fn plan(requirement: &str) -> LoadResult<JavaPlan> {
    Ok(JavaPlan {
        tool: find_jdk(requirement)?,
    })
}

pub(super) fn apply(plan: &JavaPlan, delta: &mut EnvDelta) -> LoadResult<()> {
    let jdk = &plan.tool;
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
    delta.set("JAVA_HOME", env(java_home.to_string_lossy()));
    delta.prepend("Path", env(bin.to_string_lossy()));
    Ok(())
}
