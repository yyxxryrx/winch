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

#[derive(Debug, Deserialize)]
pub struct Toolchain {
    pub msvc: Option<Msvc>,
    pub java: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub toolchain: Option<Toolchain>,
    pub dev: Option<Dev>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}
