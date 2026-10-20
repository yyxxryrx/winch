use crate::env::EnvDelta;
use crate::find;
use crate::project_config::WindowsSDK;
use crate::toolchain::{LoadError, LoadResult, WindowsSDKPlan};
use std::path::{Path, PathBuf};

fn is_exact_version(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 4 && parts.iter().all(|part| part.parse::<u32>().is_ok())
}

fn find_sdk(sdk: &WindowsSDK) -> LoadResult<find::Tool> {
    let candidates = find::find_windows_sdk()?;

    let matched = match semver::VersionReq::parse(&sdk.version) {
        Ok(requirement) => candidates
            .into_iter()
            .filter(|tool| requirement.matches(&tool.version))
            .max_by(|a, b| a.version.cmp(&b.version)),
        Err(error) => {
            if is_exact_version(&sdk.version) {
                candidates
                    .into_iter()
                    .find(|tool| tool.raw_version.as_deref() == Some(sdk.version.as_str()))
            } else {
                return Err(error.into());
            }
        }
    };

    matched.ok_or_else(|| LoadError::CannotFind("target windows sdk".to_string()))
}

fn version_text(tool: &find::Tool) -> String {
    tool.raw_version
        .clone()
        .unwrap_or_else(|| tool.version.to_string())
}

fn required_paths(root: &Path, version: &str, platform: &str) -> Vec<PathBuf> {
    let include = root.join("Include").join(version);
    let lib = root.join("Lib").join(version);

    vec![
        include.clone(),
        include.join("ucrt"),
        include.join("um"),
        include.join("shared"),
        lib.clone(),
        lib.join("ucrt").join(platform),
        lib.join("um").join(platform),
        root.join("bin").join(version).join(platform),
    ]
}

fn validate(tool: &find::Tool, platform: &str) -> LoadResult<()> {
    for path in required_paths(&tool.path, &version_text(tool), platform) {
        if !path.exists() {
            return Err(LoadError::CannotFind(path.display().to_string()));
        }
    }
    Ok(())
}

fn env(value: impl Into<String>) -> crate::env::EnvObj {
    crate::env::EnvObj::new(value.into(), "Windows SDK")
}

fn add(delta: &mut EnvDelta, key: &str, path: &Path) {
    if path.exists() {
        delta.prepend(key, env(path.to_string_lossy()));
    }
}

pub(crate) fn check(sdk: &WindowsSDK) -> LoadResult<find::Tool> {
    let tool = find_sdk(sdk)?;
    validate(&tool, sdk.platform.as_str())?;
    Ok(tool)
}

pub(super) fn plan(sdk: &WindowsSDK) -> LoadResult<WindowsSDKPlan> {
    let tool = check(sdk)?;
    Ok(WindowsSDKPlan {
        tool,
        platform: sdk.platform.clone(),
    })
}

pub(super) fn apply(plan: &WindowsSDKPlan, delta: &mut EnvDelta) -> LoadResult<()> {
    let root = &plan.tool.path;
    let version = version_text(&plan.tool);
    let platform = plan.platform.as_str();

    let include = root.join("Include").join(&version);
    let lib = root.join("Lib").join(&version);
    let bin = root.join("bin").join(&version);

    add(delta, "LIB", &lib.join("ucrt").join(platform));
    add(delta, "LIB", &lib.join("um").join(platform));

    add(delta, "INCLUDE", &include.join("cppwinrt"));
    add(delta, "INCLUDE", &include.join("winrt"));
    add(delta, "INCLUDE", &include.join("um"));
    add(delta, "INCLUDE", &include.join("shared"));
    add(delta, "INCLUDE", &include.join("ucrt"));

    add(delta, "Path", &root.join("bin").join(platform));
    add(delta, "Path", &bin.join(platform));

    add(delta, "LIBPATH", &root.join("References").join(&version));
    add(delta, "LIBPATH", &root.join("UnionMetadata").join(&version));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_version_requires_four_numeric_parts() {
        assert!(is_exact_version("10.0.26100.0"));
        assert!(!is_exact_version("10.0.26100"));
        assert!(!is_exact_version("10.0.x.0"));
        assert!(!is_exact_version("10.0"));
    }
}
