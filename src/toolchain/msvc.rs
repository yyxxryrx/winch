use super::error::{LoadError, LoadResult, ToolType};
use crate::env::EnvDelta;
use crate::msvc;
use crate::project_config::Msvc;
use log::{debug, info};

pub(super) fn apply(config: &Msvc, delta: &mut EnvDelta) -> LoadResult<()> {
    debug!("Preparing the MSVC environment");
    let version = config.version.parse::<msvc::VSVersion>()?;
    let vswhere = msvc::try_get_vswhere().ok_or(LoadError::CannotFindTool(ToolType::VSWhere))?;
    debug!("Find vswhere successfully ... {}", vswhere.display());

    let all = msvc::get_all_vs(&vswhere)?;
    let target = all
        .iter()
        .find(|v| version.is_match(&v.version))
        .ok_or_else(|| LoadError::CannotFind("target msvc".to_string()))?;
    debug!(
        "Find vs successfully ... {}",
        target.installed_path.display()
    );

    let vars = msvc::get_msvc_env(&target.installed_path, &config.platform)?;
    info!("Load msvc {}", target.version);
    delta.merge_prepend(vars);
    Ok(())
}
