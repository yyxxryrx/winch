use super::MsvcPlan;
use super::error::{LoadError, LoadResult, ToolType};
use crate::env::{EnvDelta, EnvObj};
use crate::msvc;
use crate::output;
use crate::project_config::Msvc;

fn find_vs(config: &Msvc) -> LoadResult<msvc::VS> {
    let version = config.version.parse::<msvc::VSVersion>()?;
    let vswhere = msvc::try_get_vswhere().ok_or(LoadError::CannotFindTool(ToolType::VSWhere))?;
    output::debug(format!("found vswhere at {}", vswhere.display()));

    msvc::get_all_vs(&vswhere)?
        .into_iter()
        .find(|v| version.is_match(&v.version))
        .ok_or_else(|| LoadError::CannotFind("target msvc".to_string()))
}

pub(crate) fn check(config: &Msvc) -> LoadResult<msvc::VS> {
    find_vs(config)
}

pub(super) fn plan(config: &Msvc) -> LoadResult<MsvcPlan> {
    Ok(MsvcPlan {
        vs: find_vs(config)?,
        platform: config.platform.clone(),
    })
}

pub(super) fn apply(plan: &MsvcPlan, delta: &mut EnvDelta) -> LoadResult<()> {
    output::debug("preparing the MSVC environment");
    output::debug(format!(
        "found Visual Studio at {}",
        plan.vs.installed_path.display()
    ));

    let vars = msvc::get_msvc_env(&plan.vs.installed_path, &plan.platform)?;
    output::loaded(format!("MSVC {}", plan.vs.version));

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
