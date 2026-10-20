pub(crate) mod error;
pub(crate) mod java;
pub(crate) mod lib;
pub(crate) mod msvc;
pub(crate) mod msys2;
pub(crate) mod vcpkg;

use crate::env::EnvDelta;
use crate::find::Tool;
use crate::msvc::VS;
use crate::project_config::{Config, Msys2ShellType, WindowsSDKPlatform};
use sha2::Digest;
use std::path::{Path, PathBuf};

pub use error::{LoadError, LoadResult, ToolType};

#[derive(Debug, Clone)]
pub struct WindowsSDKPlan {
    pub tool: Tool,
    pub platform: WindowsSDKPlatform,
}

#[derive(Default, Debug, Clone)]
pub struct LibPlan {
    pub windows_sdk: Option<WindowsSDKPlan>,
}

#[derive(Debug, Clone)]
pub struct MsvcPlan {
    pub vs: VS,
    pub platform: String,
}

#[derive(Debug, Clone)]
pub struct Msys2Plan {
    pub root: PathBuf,
    pub shell_type: Msys2ShellType,
}

#[derive(Debug, Clone)]
pub struct VcpkgPlan {
    pub root: PathBuf,
    pub triplet: String,
}

#[derive(Debug, Clone)]
pub struct JavaPlan {
    pub tool: Tool,
}

#[derive(Debug, Default, Clone)]
pub struct Plan {
    pub msys2: Option<Msys2Plan>,
    pub msvc: Option<MsvcPlan>,
    pub vcpkg: Option<VcpkgPlan>,
    pub java: Option<JavaPlan>,
    pub lib: LibPlan,
}

impl Plan {
    pub fn fingerprint(&self) -> String {
        let mut hasher = sha2::Sha256::new();
        add(&mut hasher, "schema", b"winch-toolchain-v1");

        if let Some(msys2) = &self.msys2 {
            add(
                &mut hasher,
                "msys2.root",
                msys2.root.to_string_lossy().as_bytes(),
            );
            add(
                &mut hasher,
                "msys2.type",
                msys2.shell_type.m_system().as_bytes(),
            );
            add(
                &mut hasher,
                "msys2.dll",
                file_stamp(&msys2.root.join("usr").join("bin").join("msys-2.0.dll")),
            );
            add(
                &mut hasher,
                "msys2.usr.local",
                msys2
                    .root
                    .join("usr")
                    .join("local")
                    .join("bin")
                    .is_dir()
                    .to_string(),
            );
            add(
                &mut hasher,
                "msys2.usr",
                msys2.root.join("usr").join("bin").is_dir().to_string(),
            );
            if let Some(dir) = msys2.shell_type.dir() {
                add(
                    &mut hasher,
                    "msys2.env",
                    msys2.root.join(dir).join("bin").is_dir().to_string(),
                );
            }
        }

        if let Some(msvc) = &self.msvc {
            add(
                &mut hasher,
                "msvc.path",
                msvc.vs.installed_path.to_string_lossy().as_bytes(),
            );
            add(
                &mut hasher,
                "msvc.version",
                msvc.vs.version.to_string().as_bytes(),
            );
            add(&mut hasher, "msvc.platform", msvc.platform.as_bytes());
            add(
                &mut hasher,
                "msvc.vcvarsall",
                file_stamp(
                    &msvc
                        .vs
                        .installed_path
                        .join("VC")
                        .join("Auxiliary")
                        .join("Build")
                        .join("vcvarsall.bat"),
                ),
            );
            add(
                &mut hasher,
                "msvc.tools",
                file_stamp(&msvc.vs.installed_path.join("VC").join("Tools").join("MSVC")),
            );
            if let Some(program_files) = std::env::var_os("ProgramFiles(x86)") {
                let kits = PathBuf::from(program_files).join("Windows Kits").join("10");
                add(
                    &mut hasher,
                    "msvc.sdk.include",
                    file_stamp(&kits.join("Include")),
                );
                add(&mut hasher, "msvc.sdk.lib", file_stamp(&kits.join("Lib")));
            } else {
                add(&mut hasher, "msvc.sdk.include", b"missing");
                add(&mut hasher, "msvc.sdk.lib", b"missing");
            }
        }

        if let Some(vcpkg) = &self.vcpkg {
            add(
                &mut hasher,
                "vcpkg.root",
                vcpkg.root.to_string_lossy().as_bytes(),
            );
            add(&mut hasher, "vcpkg.triplet", vcpkg.triplet.as_bytes());
            add(
                &mut hasher,
                "vcpkg.exe",
                file_stamp(&vcpkg.root.join("vcpkg.exe")),
            );
            let installed = vcpkg.root.join("installed").join(&vcpkg.triplet);
            add(&mut hasher, "vcpkg.installed", file_stamp(&installed));
            add(
                &mut hasher,
                "vcpkg.cmake",
                file_stamp(
                    &installed
                        .join("share")
                        .join("vcpkg")
                        .join("scripts")
                        .join("vcpkg.cmake"),
                ),
            );
        }

        if let Some(java) = &self.java {
            add(
                &mut hasher,
                "java.path",
                java.tool.path.to_string_lossy().as_bytes(),
            );
            add(
                &mut hasher,
                "java.version",
                java.tool.version.to_string().as_bytes(),
            );
            add(&mut hasher, "java.exe", file_stamp(&java.tool.path));
            if let Some(home) = java.tool.path.parent().and_then(Path::parent) {
                add(
                    &mut hasher,
                    "java.release",
                    file_stamp(&home.join("release")),
                );
            }
        }

        if let Some(windows_sdk) = &self.lib.windows_sdk {
            let version = windows_sdk
                .tool
                .raw_version
                .clone()
                .unwrap_or_else(|| windows_sdk.tool.version.to_string());
            add(
                &mut hasher,
                "sdk.root",
                windows_sdk.tool.path.to_string_lossy().as_bytes(),
            );
            add(&mut hasher, "sdk.version", version.as_bytes());
            add(
                &mut hasher,
                "sdk.platform",
                windows_sdk.platform.as_str().as_bytes(),
            );
            add(
                &mut hasher,
                "sdk.include",
                file_stamp(&windows_sdk.tool.path.join("Include").join(&version)),
            );
            add(
                &mut hasher,
                "sdk.lib",
                file_stamp(&windows_sdk.tool.path.join("Lib").join(&version)),
            );
            add(
                &mut hasher,
                "sdk.bin",
                file_stamp(&windows_sdk.tool.path.join("bin").join(&version)),
            );
        }

        hex::encode(hasher.finalize())
    }
}

fn add(hasher: &mut sha2::Sha256, label: &str, value: impl AsRef<[u8]>) {
    hasher.update(label.as_bytes());
    hasher.update([0]);
    hasher.update(value.as_ref());
    hasher.update([0]);
}

fn file_stamp(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| format!("{}.{}", duration.as_secs(), duration.subsec_nanos()))
                .unwrap_or_else(|| "unknown".to_string());
            format!("{}:{}", metadata.len(), modified)
        }
        Err(_) => "missing".to_string(),
    }
}

pub fn plan(config: &Config, msys2_hint: Option<&Path>) -> LoadResult<Plan> {
    let mut plan = Plan::default();

    if let Some(msys2) = config.toolchain.as_ref().and_then(|t| t.msys2.as_ref())
        && msys2.enable
    {
        plan.msys2 = Some(msys2::plan(msys2, msys2_hint)?);
    }

    if let Some(msvc) = config.toolchain.as_ref().and_then(|t| t.msvc.as_ref()) {
        plan.msvc = Some(msvc::plan(msvc)?);
    }

    if let Some(vcpkg) = config.dev.as_ref().and_then(|d| d.vcpkg.as_ref())
        && vcpkg.enable
    {
        plan.vcpkg = Some(vcpkg::plan(vcpkg)?);
    }

    if let Some(java) = config.toolchain.as_ref().and_then(|t| t.java.as_ref()) {
        plan.java = Some(java::plan(java)?);
    }

    plan.lib = lib::plan(&config.lib)?;

    Ok(plan)
}

pub fn apply(plan: &Plan) -> LoadResult<EnvDelta> {
    let mut delta = EnvDelta::default();

    if let Some(msys2) = &plan.msys2 {
        msys2::apply(msys2, &mut delta)?;
    }

    if let Some(msvc) = &plan.msvc {
        msvc::apply(msvc, &mut delta)?;
    }

    if let Some(vcpkg) = &plan.vcpkg {
        vcpkg::apply(vcpkg, &mut delta)?;
    }

    if let Some(java) = &plan.java {
        java::apply(java, &mut delta)?;
    }

    lib::apply(&plan.lib, &mut delta)?;

    Ok(delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msvc_plan(path: &str) -> MsvcPlan {
        MsvcPlan {
            vs: VS {
                version: "17.9.1.0".parse().unwrap(),
                installed_path: PathBuf::from(path),
            },
            platform: "x64".to_string(),
        }
    }

    #[test]
    fn fingerprint_is_stable() {
        let plan = Plan {
            msvc: Some(msvc_plan("C:/VS")),
            ..Default::default()
        };
        assert_eq!(plan.fingerprint(), plan.fingerprint());
    }

    #[test]
    fn fingerprint_changes_with_toolchain_data() {
        let plan = Plan {
            msvc: Some(msvc_plan("C:/VS")),
            ..Default::default()
        };
        let mut other = plan.clone();
        other.msvc.as_mut().unwrap().platform = "x86".to_string();
        assert_ne!(plan.fingerprint(), other.fingerprint());
    }

    #[test]
    fn empty_plan_fingerprint_is_stable() {
        let plan = Plan::default();
        assert_eq!(plan.fingerprint(), plan.fingerprint());
    }

    fn sdk_plan(platform: WindowsSDKPlatform) -> WindowsSDKPlan {
        WindowsSDKPlan {
            tool: Tool {
                path: PathBuf::from("C:/Kits"),
                version: "10.0.26100".parse().unwrap(),
                raw_version: Some("10.0.26100.0".to_string()),
            },
            platform,
        }
    }

    #[test]
    fn fingerprint_changes_with_windows_sdk() {
        let mut plan = Plan::default();
        plan.lib.windows_sdk = Some(sdk_plan(WindowsSDKPlatform::X64));
        let first = plan.fingerprint();

        plan.lib.windows_sdk.as_mut().unwrap().platform = WindowsSDKPlatform::X86;
        assert_ne!(first, plan.fingerprint());
    }
}
