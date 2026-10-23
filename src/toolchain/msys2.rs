use super::Msys2Plan;
use crate::env::EnvDelta;
use crate::output;
use crate::project_config::Msys2;
use crate::toolchain::{LoadError, LoadResult};
use std::path::{Path, PathBuf};

fn find_root() -> LoadResult<PathBuf> {
    crate::msys2::find_msys2().ok_or_else(|| LoadError::CannotFind("msys2 root".to_string()))
}

fn env(value: impl Into<String>) -> crate::env::EnvObj {
    crate::env::EnvObj::new(value.into(), "msys2")
}

pub(crate) fn check() -> LoadResult<PathBuf> {
    find_root()
}

pub(super) fn plan(config: &Msys2, hint: Option<&Path>) -> LoadResult<Msys2Plan> {
    let root = crate::msys2::find_msys2_with_hint(hint)
        .ok_or_else(|| LoadError::CannotFind("msys2 root".to_string()))?;
    Ok(Msys2Plan {
        root,
        shell_type: config.shell_type,
    })
}

pub(super) fn apply(plan: &Msys2Plan, delta: &mut EnvDelta) -> LoadResult<()> {
    output::loaded("MSYS2");
    let root = &plan.root;

    delta.set("MSYSTEM", env(plan.shell_type.m_system()));
    if !plan.shell_type.mingw_package_prefix().is_empty() {
        delta.set(
            "MINGW_PACKAGE_PREFIX",
            env(plan.shell_type.mingw_package_prefix()),
        );
    }

    let mut paths = vec![
        root.join("usr").join("local").join("bin"),
        root.join("usr").join("bin"),
    ];
    if let Some(dir) = plan.shell_type.dir() {
        paths.push(root.join(dir).join("bin"));
    }

    for path in paths.iter().rev().filter(|path| path.exists()) {
        delta.prepend("Path", env(path.to_string_lossy()));
    }

    Ok(())
}
