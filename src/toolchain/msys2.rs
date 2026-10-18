use crate::env::EnvDelta;
use crate::project_config::Msys2;
use crate::toolchain::{LoadError, LoadResult};
use log::info;
use std::path::PathBuf;

fn find_root() -> LoadResult<PathBuf> {
    crate::msys2::find_msys2().ok_or_else(|| LoadError::CannotFind("msys2 root".to_string()))
}

fn env(value: impl Into<String>) -> crate::env::EnvObj {
    crate::env::EnvObj::new(value.into(), "msys2")
}

pub(crate) fn check() -> LoadResult<PathBuf> {
    find_root()
}

pub(super) fn apply(config: &Msys2, delta: &mut EnvDelta) -> LoadResult<()> {
    info!("Load msys2");
    let root = find_root()?;

    delta.set("MSYSTEM", env(config.shell_type.m_system()));
    if !config.shell_type.mingw_package_prefix().is_empty() {
        delta.set(
            "MINGW_PACKAGE_PREFIX",
            env(config.shell_type.mingw_package_prefix()),
        );
    }

    let mut paths = vec![
        root.join("usr").join("local").join("bin"),
        root.join("usr").join("bin"),
    ];
    if let Some(dir) = config.shell_type.dir() {
        paths.push(root.join(dir).join("bin"));
    }

    for path in paths.iter().rev().filter(|path| path.exists()) {
        delta.prepend("Path", env(path.to_string_lossy()));
    }

    Ok(())
}
