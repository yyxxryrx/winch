use crate::env::EnvDelta;
use crate::project_config::Lib;
use crate::toolchain::{LibPlan, LoadResult};

pub(crate) mod windows_sdk;

pub(super) fn plan(lib: &Lib) -> LoadResult<LibPlan> {
    let mut plan = LibPlan::default();

    if let Some(windows_sdk) = lib.windows_sdk.as_ref() {
        plan.windows_sdk = Some(windows_sdk::plan(windows_sdk)?);
    }

    Ok(plan)
}

pub(super) fn apply(lib: &LibPlan, delta: &mut EnvDelta) -> LoadResult<()> {
    if let Some(windows_sdk) = &lib.windows_sdk {
        windows_sdk::apply(windows_sdk, delta)?;
    }
    Ok(())
}
