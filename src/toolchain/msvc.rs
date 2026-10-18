use super::error::{LoadError, LoadResult, ToolType};
use crate::env::{EnvDelta, EnvObj};
use crate::msvc;
use crate::project_config::Msvc;
use log::{debug, info};

fn find_vs(config: &Msvc) -> LoadResult<msvc::VS> {
    let version = config.version.parse::<msvc::VSVersion>()?;
    let vswhere = msvc::try_get_vswhere().ok_or(LoadError::CannotFindTool(ToolType::VSWhere))?;
    debug!("Find vswhere successfully ... {}", vswhere.display());

    msvc::get_all_vs(&vswhere)?
        .into_iter()
        .find(|v| version.is_match(&v.version))
        .ok_or_else(|| LoadError::CannotFind("target msvc".to_string()))
}

pub(crate) fn check(config: &Msvc) -> LoadResult<msvc::VS> {
    find_vs(config)
}

pub(super) fn apply(config: &Msvc, delta: &mut EnvDelta) -> LoadResult<()> {
    debug!("Preparing the MSVC environment");
    let target = find_vs(config)?;
    debug!(
        "Find vs successfully ... {}",
        target.installed_path.display()
    );

    let vars = msvc::get_msvc_env(&target.installed_path, &config.platform)?;
    info!("Load msvc {}", target.version);

    const PREPEND_VARS: [&str; 5] = ["Path", "LIB", "INCLUDE", "LIBPATH", "EXTERNAL_INCLUDE"];
    for (key, value) in vars {
        if PREPEND_VARS.contains(&key.as_str()) {
            delta.prepend(key, EnvObj::new(value, "msvc"));
        } else {
            delta.set(key, EnvObj::new(value, "msvc"));
        }
    }
    Ok(())
}
