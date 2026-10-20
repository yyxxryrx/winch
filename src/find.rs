use crate::version::{self, GetVersionResult};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Tool {
    pub path: PathBuf,
    pub version: semver::Version,
    pub raw_version: Option<String>,
}

impl Tool {
    pub fn version_text(&self) -> String {
        self.raw_version
            .clone()
            .unwrap_or_else(|| self.version.to_string())
    }
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
        .map(|path| {
            get_version(&path).map(|version| Tool {
                version,
                path,
                raw_version: None,
            })
        })
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

fn parse_sdk_version(name: &str) -> Option<(semver::Version, String)> {
    let parts = name.split('.').collect::<Vec<_>>();
    if parts.len() != 4 {
        return None;
    }

    let major = parts[0].parse::<u64>().ok()?;
    let minor = parts[1].parse::<u64>().ok()?;
    let patch = parts[2].parse::<u64>().ok()?;
    parts[3].parse::<u64>().ok()?;

    Some((semver::Version::new(major, minor, patch), name.to_string()))
}

pub fn find_windows_sdk() -> GetVersionResult<Vec<Tool>> {
    use winreg::HKLM;

    let mut roots = Vec::new();
    if let Ok(root) = HKLM.open_subkey(r"SOFTWARE\WOW6432Node\Microsoft\Microsoft SDKs\Windows") {
        for name in root.enum_keys().flatten() {
            let Ok(key) = root.open_subkey(&name) else {
                continue;
            };
            let install_dir: String = key.get_value("InstallationFolder").unwrap_or_default();
            if !install_dir.is_empty() {
                roots.push(PathBuf::from(install_dir));
            }
        }
    }
    if let Some(program_files) = std::env::var_os("ProgramFiles(x86)") {
        roots.push(PathBuf::from(program_files).join("Windows Kits").join("10"));
    }

    let mut sdks = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root.join("Include")) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }

            let name = entry.file_name().to_string_lossy().into_owned();
            let Some((version, raw_version)) = parse_sdk_version(&name) else {
                continue;
            };
            if !root.join("Lib").join(&name).is_dir() || !root.join("bin").join(&name).is_dir() {
                continue;
            }
            if sdks.iter().any(|tool: &Tool| {
                tool.path == root && tool.raw_version.as_deref() == Some(raw_version.as_str())
            }) {
                continue;
            }

            sdks.push(Tool {
                path: root.clone(),
                version,
                raw_version: Some(raw_version),
            });
        }
    }

    sdks.sort_by(|a, b| b.version.cmp(&a.version));
    Ok(sdks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_four_part_sdk_version() {
        let (version, raw) = parse_sdk_version("10.0.26100.0").unwrap();
        assert_eq!(version, semver::Version::new(10, 0, 26100));
        assert_eq!(raw, "10.0.26100.0");
    }

    #[test]
    fn rejects_invalid_sdk_versions() {
        assert!(parse_sdk_version("10.0.26100").is_none());
        assert!(parse_sdk_version("10.0.x.0").is_none());
        assert!(parse_sdk_version("not-a-version").is_none());
    }
}
