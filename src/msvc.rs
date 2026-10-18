use crate::find::find_tool;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn try_get_vswhere() -> Option<PathBuf> {
    find_tool("vswhere.exe").into_iter().next().or_else(|| {
        std::env::var_os("ProgramFiles(x86)")
            .map(|root| {
                PathBuf::from(root)
                    .join("Microsoft Visual Studio")
                    .join("Installer")
                    .join("vswhere.exe")
            })
            .filter(|path| path.exists())
    })
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("IOError: {0}")]
    IOError(#[from] std::io::Error),
    #[error("ParseError: {0}")]
    ParseError(#[from] json::Error),
    #[error("Not success: exit code {0}")]
    NotSuccess(i32),
    #[error("Unexpected vswhere output")]
    UnexpectedOutput,
}

fn run_and_get_output(program: &Path, args: &[&str]) -> Result<String, RunError> {
    let output = std::process::Command::new(program).args(args).output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(RunError::NotSuccess(output.status.code().unwrap_or(-1)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VSVersion {
    major: u32,
    minor: u32,
    build: u32,
    revision: u32,
}

impl VSVersion {
    pub fn is_match(&self, installed: &VSVersion) -> bool {
        self.major == installed.major
            && (self.minor == 0 || self.minor == installed.minor)
            && (self.build == 0 || self.build == installed.build)
            && (self.revision == 0 || self.revision == installed.revision)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VersionParseError {
    #[error("SegmentCount: expected at most 4 sections, got {0}")]
    SegmentCount(usize),
    #[error("ParseError: {0}")]
    ParseError(#[from] std::num::ParseIntError),
}

impl std::str::FromStr for VSVersion {
    type Err = VersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s
            .split('.')
            .map(str::parse::<u32>)
            .collect::<Result<Vec<_>, _>>()?;
        if parts.is_empty() || parts.len() > 4 {
            return Err(VersionParseError::SegmentCount(parts.len()));
        }

        let mut parts = parts.into_iter();
        Ok(Self {
            major: parts.next().unwrap_or_default(),
            minor: parts.next().unwrap_or_default(),
            build: parts.next().unwrap_or_default(),
            revision: parts.next().unwrap_or_default(),
        })
    }
}

impl std::fmt::Display for VSVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.major, self.minor, self.build, self.revision
        )
    }
}

#[derive(Debug, Clone)]
pub struct VS {
    pub version: VSVersion,
    pub installed_path: PathBuf,
}

pub fn get_all_vs(vswhere: &Path) -> Result<Vec<VS>, RunError> {
    let output = run_and_get_output(vswhere, &["-all", "-format", "json"])?;
    let value = json::parse(output.trim())?;
    let json::JsonValue::Array(entries) = value else {
        return Err(RunError::UnexpectedOutput);
    };

    Ok(entries
        .iter()
        .filter_map(|entry| {
            let version = entry["installationVersion"].as_str()?.parse().ok()?;
            let installed_path = entry["installationPath"].as_str()?;
            Some(VS {
                version,
                installed_path: PathBuf::from(installed_path),
            })
        })
        .collect())
}

#[derive(Debug, thiserror::Error)]
pub enum GetEnvError {
    #[error("Bat not found")]
    BatNotFound,
    #[error("IOError: {0}")]
    IOError(#[from] std::io::Error),
    #[error("vcvarsall.bat exited with code {0}")]
    NotSuccess(i32),
}

const MARKER: &str = "----------";

const MSVC_VARS: [&str; 21] = [
    "Path",
    "WindowsLibPath",
    "FSHARPINSTALLDIR",
    "VCToolsInstallDir",
    "WindowsSDK_ExecutablePath_x86",
    "VCIDEInstallDir",
    "WindowsSdkBinPath",
    "VCToolsRedistDir",
    "VCINSTALLDIR",
    "FrameworkDir",
    "LIBPATH",
    "FrameworkVersion32",
    "WindowsSdkVerBinPath",
    "WindowsSDK_ExecutablePath_x64",
    "DevEnvDir",
    "VSINSTALLDIR",
    "EXTERNAL_INCLUDE",
    "LIB",
    "WindowsSdkDir",
    "INCLUDE",
    "Platform",
];

pub fn get_msvc_env(
    vs_path: &Path,
    platform: &str,
) -> Result<HashMap<String, String>, GetEnvError> {
    let build_dir = vs_path.join("VC").join("Auxiliary").join("Build");
    if !build_dir.join("vcvarsall.bat").exists() {
        return Err(GetEnvError::BatNotFound);
    }

    let command = format!("call vcvarsall.bat {platform} && echo {MARKER} && set");
    let output = std::process::Command::new("cmd.exe")
        .current_dir(&build_dir)
        .env("Path", "C:\\Windows\\system32;C:\\Windows")
        .args(["/C", command.as_str()])
        .output()?;
    if !output.status.success() {
        return Err(GetEnvError::NotSuccess(output.status.code().unwrap_or(-1)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut env = stdout
        .lines()
        .map(str::trim)
        .skip_while(|line| *line != MARKER)
        .skip(1)
        .filter(|line| !line.is_empty())
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| MSVC_VARS.contains(key))
        .map(|(key, value)| {
            if key == "Path" {
                (
                    key.to_string(),
                    value.replace("C:\\Windows\\system32;C:\\Windows", ""),
                )
            } else {
                (key.to_string(), value.to_string())
            }
        })
        .collect::<HashMap<_, _>>();

    const PATH_LIST_VARS: [&str; 5] = ["Path", "LIB", "INCLUDE", "LIBPATH", "EXTERNAL_INCLUDE"];
    for key in PATH_LIST_VARS {
        if let Some(value) = env.get_mut(key) {
            *value = value
                .split(';')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(";");
        }
    }
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_version() {
        let version: VSVersion = "17.9.34728.123".parse().unwrap();
        assert_eq!(version.to_string(), "17.9.34728.123");
    }

    #[test]
    fn parse_partial_version() {
        let version: VSVersion = "17.9".parse().unwrap();
        assert_eq!(version.to_string(), "17.9.0.0");
    }

    #[test]
    fn too_many_segments_is_rejected() {
        assert!("1.2.3.4.5".parse::<VSVersion>().is_err());
    }

    #[test]
    fn zero_segments_are_wildcards() {
        let requested: VSVersion = "17.0".parse().unwrap();
        let installed: VSVersion = "17.9.34728.123".parse().unwrap();
        assert!(requested.is_match(&installed));

        let requested: VSVersion = "17.10".parse().unwrap();
        assert!(!requested.is_match(&installed));
    }
}
