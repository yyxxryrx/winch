use crate::version::{self, GetVersionResult};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Tool {
    pub path: PathBuf,
    pub version: semver::Version,
}

fn find_tool_from_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .find(|entry| entry.file_name() == name)
        .map(|entry| entry.path())
}

pub fn find_tool(name: &str) -> Vec<PathBuf> {
    std::env::var_os("Path")
        .map(|paths| {
            std::env::split_paths(&paths)
                .filter_map(|dir| find_tool_from_dir(&dir, name))
                .collect()
        })
        .unwrap_or_default()
}

fn find_with_version(
    name: &str,
    get_version: fn(&Path) -> GetVersionResult<semver::Version>,
) -> GetVersionResult<Vec<Tool>> {
    find_tool(name)
        .into_iter()
        .map(|path| get_version(&path).map(|version| Tool { version, path }))
        .collect()
}

pub fn find_clang_with_version() -> GetVersionResult<Vec<Tool>> {
    find_with_version("clang.exe", version::get_clang_version)
}

pub fn find_gcc_with_version() -> GetVersionResult<Vec<Tool>> {
    find_with_version("gcc.exe", version::get_gcc_version)
}

pub fn find_jdk_with_version() -> GetVersionResult<Vec<Tool>> {
    find_with_version("java.exe", version::get_jdk_version)
}

pub fn find_vcpkg() -> Option<PathBuf> {
    std::env::var_os("VCPKG_ROOT")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .or_else(|| {
            find_tool("vcpkg.exe")
                .first()
                .and_then(|path| path.parent())
                .map(Path::to_path_buf)
        })
}
