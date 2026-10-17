use crate::SPLITTER;
use std::path::{Path, PathBuf};

pub fn find_tool_from_dir<P>(name: &str, path: P) -> Option<PathBuf>
where
    P: AsRef<Path>,
{
    path.as_ref()
        .read_dir()
        .map(|mut r| r.find(|e| e.as_ref().is_ok_and(|s| s.file_name() == name)))
        .map(|e| e.and_then(|e| e.map_or(None, |e| Some(e.path()))))
        .unwrap_or_default()
}

pub fn find_tool(name: &str) -> Vec<PathBuf> {
    let path_var = std::env::var("Path").unwrap_or_default();
    path_var
        .split(SPLITTER)
        .filter_map(|s| find_tool_from_dir(name, s))
        .collect()
}

#[derive(Debug)]
pub struct Tool {
    pub path: PathBuf,
    pub version: semver::Version,
}

pub fn find_clang_with_version() -> crate::version::GetVersionResult<Vec<Tool>> {
    find_tool("clang.exe")
        .into_iter()
        .map(|p| {
            crate::version::get_clang_version(&p).map(|v| Tool {
                version: v,
                path: p,
            })
        })
        .collect()
}

pub fn find_gcc_with_version() -> crate::version::GetVersionResult<Vec<Tool>> {
    find_tool("gcc.exe")
        .into_iter()
        .map(|p| {
            crate::version::get_gcc_version(&p).map(|v| Tool {
                version: v,
                path: p,
            })
        })
        .collect()
}

pub fn find_jdk_with_version() -> crate::version::GetVersionResult<Vec<Tool>> {
    find_tool("java.exe")
        .into_iter()
        .map(|p| {
            crate::version::get_jdk_version(&p).map(|v| Tool {
                version: v,
                path: p,
            })
        })
        .collect()
}

pub fn find_vcpkg() -> Option<PathBuf> {
    std::env::var("VCPKG_ROOT")
        .map_or(None, |v| Some(PathBuf::from(v)))
        .or(find_tool("vcpkg.exe")
            .first()
            .and_then(|p| p.parent().map(|p| p.to_path_buf())))
}
