use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Display;

#[derive(Debug, Deserialize, JsonSchema, Default, Clone)]
#[serde(rename_all = "lowercase")]
#[schemars(description = "Windows SDK platform.")]
pub enum WindowsSDKPlatform {
    #[default]
    X64,
    X86,
    Arm64,
}

impl Display for WindowsSDKPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl WindowsSDKPlatform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::X86 => "x86",
            Self::Arm64 => "arm64",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WindowsSDK {
    #[schemars(
        description = "Windows SDK semver requirement (for example 10.0) or exact 4-part version (for example 10.0.26100.0)."
    )]
    pub version: String,
    #[serde(default)]
    #[schemars(description = "SDK platform: x64, x86 or arm64; defaults to x64.")]
    pub platform: WindowsSDKPlatform,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
#[schemars(description = "Library and SDK selection.")]
pub struct Lib {
    #[schemars(description = "Windows SDK selection.")]
    pub windows_sdk: Option<WindowsSDK>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Vcpkg {
    #[serde(default)]
    #[schemars(description = "Enable vcpkg integration.")]
    pub enable: bool,
    #[schemars(description = "vcpkg triplet, for example x64-windows.")]
    pub triplet: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Dev {
    pub vcpkg: Option<Vcpkg>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Msvc {
    #[schemars(
        description = "Visual Studio version, up to 4 dot-separated segments; omitted segments act as wildcards."
    )]
    pub version: String,
    #[schemars(description = "Platform passed to vcvarsall, such as x64, x86 or arm64.")]
    pub platform: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Default, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(description = "MSYS2 environment type.")]
pub enum Msys2ShellType {
    MinGW32,
    MinGW64,
    #[default]
    UCrt64,
    Clang64,
    Msys2,
    ClangArm64,
}

impl Msys2ShellType {
    pub fn mingw_package_prefix(&self) -> &'static str {
        match self {
            Self::UCrt64 => "mingw-w64-ucrt-x86_64",
            Self::MinGW64 => "mingw-w64-x86_64",
            Self::MinGW32 => "mingw-w64-i686",
            Self::Clang64 => "mingw-w64-clang-x86_64",
            Self::ClangArm64 => "mingw-w64-clang-aarch64",
            Self::Msys2 => "",
        }
    }

    pub fn m_system(&self) -> &'static str {
        match self {
            Self::MinGW64 => "MINGW64",
            Self::MinGW32 => "MINGW32",
            Self::UCrt64 => "UCRT64",
            Self::Msys2 => "MSYS",
            Self::Clang64 => "CLANG64",
            Self::ClangArm64 => "CLANGARM64",
        }
    }

    pub fn dir(&self) -> Option<&'static str> {
        match self {
            Self::UCrt64 => Some("ucrt64"),
            Self::MinGW32 => Some("mingw32"),
            Self::MinGW64 => Some("mingw64"),
            Self::Clang64 => Some("clang64"),
            Self::ClangArm64 => Some("clangarm64"),
            Self::Msys2 => None,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Msys2 {
    #[serde(default)]
    #[schemars(description = "Enable MSYS2 integration.")]
    pub enable: bool,
    #[serde(default, rename = "type")]
    #[schemars(description = "MSYS2 environment type; defaults to ucrt64.")]
    pub shell_type: Msys2ShellType,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Toolchain {
    pub msvc: Option<Msvc>,
    pub msys2: Option<Msys2>,
    #[schemars(description = "JDK semver requirement, for example 17 or >=17, <21.")]
    pub java: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[schemars(
    description = "Custom environment variables. Plain keys are set; prepend/append extend existing values."
)]
pub struct EnvConfig {
    #[serde(flatten)]
    pub set: HashMap<String, String>,
    #[serde(default)]
    #[schemars(description = "Variables to prepend to existing values.")]
    pub prepend: HashMap<String, String>,
    #[serde(default)]
    #[schemars(description = "Variables to append to existing values.")]
    pub append: HashMap<String, String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, description = "winch project configuration.")]
pub struct Config {
    #[serde(default)]
    #[schemars(description = "Library configuration.")]
    pub lib: Lib,
    #[schemars(description = "Toolchain selection.")]
    pub toolchain: Option<Toolchain>,
    #[schemars(description = "Development integrations.")]
    pub dev: Option<Dev>,
    #[serde(default)]
    #[schemars(description = "Custom environment variables.")]
    pub env: EnvConfig,
    #[serde(default)]
    #[schemars(
        description = "Only keep resolved variables and pure_keep; do not inherit the caller environment."
    )]
    pub pure: bool,
    #[serde(default)]
    #[schemars(
        description = "Variables to keep in pure mode. Omit to use the built-in whitelist; an empty array keeps none."
    )]
    pub pure_keep: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_defaults_to_false() {
        let config: Config = toml::from_str("").unwrap();
        assert!(!config.pure);
    }

    #[test]
    fn parses_pure_flag() {
        let config: Config = toml::from_str("pure = true\n").unwrap();
        assert!(config.pure);
    }

    #[test]
    fn pure_keep_defaults_to_none() {
        let config: Config = toml::from_str("pure = true\n").unwrap();
        assert!(config.pure_keep.is_none());
    }

    #[test]
    fn pure_keep_can_be_empty() {
        let config: Config = toml::from_str("pure_keep = []\n").unwrap();
        assert_eq!(config.pure_keep, Some(Vec::new()));
    }

    #[test]
    fn parses_pure_keep_list() {
        let config: Config = toml::from_str("pure_keep = [\"SystemRoot\", \"TEMP\"]\n").unwrap();
        assert_eq!(
            config.pure_keep,
            Some(vec!["SystemRoot".to_string(), "TEMP".to_string()])
        );
    }

    #[test]
    fn env_keeps_plain_values_as_set() {
        let config: Config = toml::from_str("[env]\nFOO = \"bar\"\n").unwrap();
        assert_eq!(config.env.set["FOO"], "bar");
        assert!(config.env.prepend.is_empty());
        assert!(config.env.append.is_empty());
    }

    #[test]
    fn env_parses_prepend_and_append() {
        let config: Config = toml::from_str(
            r#"
[env]
FOO = "bar"

[env.prepend]
PATH = "C:\\prepend"

[env.append]
PATH = "C:\\append"
"#,
        )
        .unwrap();
        assert_eq!(config.env.set["FOO"], "bar");
        assert_eq!(config.env.prepend["PATH"], r"C:\prepend");
        assert_eq!(config.env.append["PATH"], r"C:\append");
    }
}
