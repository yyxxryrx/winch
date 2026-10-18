use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Vcpkg {
    #[serde(default)]
    pub enable: bool,
    pub triplet: String,
}

#[derive(Debug, Deserialize)]
pub struct Dev {
    pub vcpkg: Option<Vcpkg>,
}

#[derive(Debug, Deserialize)]
pub struct Msvc {
    pub version: String,
    pub platform: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
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

#[derive(Debug, Deserialize)]
pub struct Msys2 {
    #[serde(default)]
    pub enable: bool,
    #[serde(default, rename = "type")]
    pub shell_type: Msys2ShellType,
}

#[derive(Debug, Deserialize)]
pub struct Toolchain {
    pub msvc: Option<Msvc>,
    pub msys2: Option<Msys2>,
    pub java: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct EnvConfig {
    #[serde(flatten)]
    pub set: HashMap<String, String>,
    #[serde(default)]
    pub prepend: HashMap<String, String>,
    #[serde(default)]
    pub append: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub toolchain: Option<Toolchain>,
    pub dev: Option<Dev>,
    #[serde(default)]
    pub env: EnvConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

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
