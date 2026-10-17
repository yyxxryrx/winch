use crate::project_config::Config;
use crate::util::{env_add, env_merge};
use crate::{find, msvc};
use colored::Colorize;
use log::{debug, error, info};
use std::collections::HashMap;

pub fn load_config(config: &Config) -> error::LoadResult<HashMap<String, String>> {
    let mut envs = HashMap::new();

    if let Some(msvc) = config.toolchain.as_ref().and_then(|t| t.msvc.as_ref()) {
        debug!("Preparing the MSVC environment");
        let version = msvc.version.parse::<msvc::VSVersion>()?;
        let Some(vswhere) = msvc::try_get_vswhere() else {
            error!("Cannot find vswhere.exe");
            eprintln!("{}", "Cannot found vswhere".bright_red());
            eprintln!("{}", "You can install it with winget or scoop".bright_red());
            eprintln!(
                "{}",
                " - For winget: winget install --id Microsoft.VisualStudio.Locator --source winget"
                    .bright_red()
            );
            eprintln!("{}", " - For scoop: scoop install vswhere".bright_red());
            return Err(error::LoadError::CannotFindTool(error::ToolType::VSWhere));
        };
        debug!("Find vswhere successfully ... {}", vswhere.display());
        let vs = msvc::get_all_vs(&vswhere)?;
        let Some(v) = vs.iter().find(|v| version.is_match(&v.version)) else {
            return Err(error::LoadError::CannotFind("target msvc".to_string()));
        };
        debug!("Find vs successfully ... {}", v.installed_path.display());
        let vars = msvc::get_msvc_env(&v.installed_path, &msvc.platform)?;
        debug!("Find msvc successfully");
        info!("Load msvc {}", v.version);
        env_merge(&mut envs, &vars);
    }

    if let Some(vcpkg) = config.dev.as_ref().and_then(|d| d.vcpkg.as_ref())
        && vcpkg.enable
    {
        let Some(vcpkg_root) = find::find_vcpkg() else {
            return Err(error::LoadError::CannotFind("vcpkg root".to_string()));
        };
        debug!("Find vcpkg root successfully ... {}", vcpkg_root.display());
        info!("Load VCPKG");
        envs.insert(String::from("VCPKG_ROOT"), vcpkg_root.display().to_string());
        env_add(
            &mut envs,
            "LIB",
            &vcpkg_root
                .join("installed")
                .join(&vcpkg.triplet)
                .join("lib")
                .to_string_lossy(),
        );
        env_add(
            &mut envs,
            "Path",
            &vcpkg_root
                .join("installed")
                .join(&vcpkg.triplet)
                .join("bin")
                .to_string_lossy(),
        );
        env_add(
            &mut envs,
            "INCLUDE",
            &vcpkg_root
                .join("installed")
                .join(&vcpkg.triplet)
                .join("include")
                .to_string_lossy(),
        );
    }

    if let Some(java) = config.toolchain.as_ref().and_then(|d| d.java.as_ref()) {
        let version = semver::VersionReq::parse(java)?;
        let Some(jdk) = find::find_jdk_with_version()?
            .into_iter()
            .find(|t| version.matches(&t.version))
        else {
            return Err(error::LoadError::CannotFind("target java".to_string()));
        };
        debug!("Select java ... {}", jdk.path.display());

        let Some(java_home) = jdk.path.parent().and_then(|p| p.parent()) else {
            return Err(error::LoadError::GetFailed("JAVA_HOME".to_string()));
        };
        debug!("Get JAVA_HOME ... {}", java_home.display());

        info!("Load java");
        envs.insert(String::from("JAVA_HOME"), java_home.display().to_string());
        env_add(
            &mut envs,
            "Path",
            jdk.path.parent().unwrap().to_str().unwrap(),
        );
    }
    Ok(envs)
}

pub mod error {
    use thiserror::Error;

    use crate::{
        msvc::{GetEnvError, RunError, VersionParseError},
        version::GetVersionError,
    };

    pub type LoadResult<T> = Result<T, LoadError>;

    #[derive(Debug)]
    pub enum ToolType {
        VSWhere,
    }

    impl std::fmt::Display for ToolType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::VSWhere => write!(f, "vswhere"),
            }
        }
    }

    #[derive(Debug, Error)]
    pub enum LoadError {
        #[error("Cannot find {0} tool")]
        CannotFindTool(ToolType),
        #[error("Cannot find {0}")]
        CannotFind(String),
        #[error("Get {0} failed")]
        GetFailed(String),
        #[error("ConvertError: {0}")]
        ConvertError(#[from] VersionParseError),
        #[error("RunError: {0}")]
        RunError(#[from] RunError),
        #[error("GetVersionError: {0}")]
        GetVersionError(#[from] GetVersionError),
        #[error("GetEnvError: {0}")]
        GetEnvError(#[from] GetEnvError),
        #[error("ParseVersionReqError: {0}")]
        ParseVersionReqError(#[from] semver::Error),
    }
}

