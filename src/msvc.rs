use crate::find::find_tool;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn try_get_vswhere() -> Option<PathBuf> {
    find_tool("vswhere.exe").into_iter().next().or_else(|| {
        std::env::var("ProgramFiles(x86)").map_or(None, |v| {
            let p = PathBuf::from(v)
                .join("Microsoft Visual Studio")
                .join("Installer")
                .join("vswhere.exe");
            if p.exists() { Some(p) } else { None }
        })
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
}

fn run_and_get_output<'a>(program: &'a Path, args: &'a [&str]) -> Result<String, RunError> {
    let output = std::process::Command::new(program).args(args).output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(RunError::NotSuccess(output.status.code().unwrap_or(-1)))
    }
}

#[derive(Debug, PartialEq, PartialOrd)]
pub struct VSVersion {
    major: u32,
    minor: u32,
    build: u32,
    revision: u32,
}

impl VSVersion {
    pub fn is_match(&self, version: &VSVersion) -> bool {
        self.major == version.major
            && (self.minor == version.minor || self.minor == 0)
            && (self.build == version.build || self.build == 0)
            && (self.revision == version.revision || self.revision == 0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VersionParseError {
    #[error("SegmentCount: Number of sections does not match, expected 4, got {0}")]
    SegmentCount(usize),
    #[error("ParseError: {0}")]
    ParseError(#[from] std::num::ParseIntError),
}

impl std::str::FromStr for VSVersion {
    type Err = VersionParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let seg = s.split('.').collect::<Vec<_>>();
        if seg.len() > 4 {
            return Err(VersionParseError::SegmentCount(seg.len()));
        }
        let num = seg.iter().map(|s| s.parse::<u32>()).collect::<Vec<_>>();

        for n in num.iter() {
            n.clone()?;
        }

        Ok(Self {
            major: num
                .first()
                .map(|r| match r {
                    Ok(r) => *r,
                    _ => unreachable!(),
                })
                .unwrap_or_default(),
            minor: num
                .get(1)
                .map(|r| match r {
                    Ok(r) => *r,
                    _ => unreachable!(),
                })
                .unwrap_or_default(),
            build: num
                .get(2)
                .map(|r| match r {
                    Ok(r) => *r,
                    _ => unreachable!(),
                })
                .unwrap_or_default(),
            revision: num
                .get(3)
                .map(|r| match r {
                    Ok(r) => *r,
                    _ => unreachable!(),
                })
                .unwrap_or_default(),
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

#[derive(Debug)]
pub struct VS {
    pub version: VSVersion,
    pub installed_path: PathBuf,
}

pub fn get_all_vs(vswhere: &Path) -> Result<Vec<VS>, RunError> {
    let output = run_and_get_output(vswhere, &["-all", "-format", "json"])?;

    let v = json::parse(output.trim())?;
    match v {
        json::JsonValue::Array(arr) => Ok(arr
            .into_iter()
            .filter(|v| v.has_key("installationVersion") && v.has_key("installationPath"))
            .map(|mut v| VS {
                version: v["installationVersion"]
                    .take_string()
                    .unwrap()
                    .parse()
                    .unwrap(),
                installed_path: v["installationPath"].take_string().unwrap().into(),
            })
            .collect()),
        _ => unreachable!(),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetEnvError {
    #[error("Bat not found")]
    BatNotFound,
}

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
    let output = std::process::Command::new("cmd.exe")
        .current_dir(build_dir)
        .args([
            "/C",
            &format!("call vcvarsall.bat {platform} && echo ---------- && set"),
        ])
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&output.stdout);
    let mut out = out
        .split('\n')
        .map(|line| line.trim())
        .skip_while(|&line| line != "----------")
        .skip(2)
        .filter(|line| !line.is_empty())
        .flat_map(|line| line.split_once("="))
        .filter(|(k, _)| MSVC_VARS.contains(k))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<HashMap<_, _>>();
    let count = std::env::var("Path")
        .map(|p| p.split(';').count())
        .unwrap_or_default();
    let msvc_count = out.get("Path").map(|s| s.split(';').count());
    if msvc_count.is_some_and(|c| c > count) {
        out.insert(
            "Path".to_string(),
            out.get("Path")
                .unwrap()
                .split(';')
                .take(msvc_count.unwrap() - count)
                .collect::<Vec<_>>()
                .join(";"),
        );
    }
    Ok(out)
}
