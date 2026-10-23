use crate::env::{EnvDelta, env_to_string};
use crate::project_config::Config;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;

const PRESETS_VERSION: u32 = 3;
const DEFAULT_CONFIG: &str = "winch.toml";

fn preset_prefix(config_path: &Path) -> String {
    if config_path.file_name() == Some(OsStr::new(DEFAULT_CONFIG)) {
        return "winch".to_string();
    }

    let stem = config_path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("config");
    let stem = stem.strip_prefix("winch").unwrap_or(stem);
    let stem = stem.trim_matches(|c| matches!(c, '-' | '.' | '_'));
    let stem = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();

    if stem.is_empty() {
        "winch-config".to_string()
    } else {
        format!("winch-{stem}")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    #[error("IOError: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSONError: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Preset file root must be a JSON object")]
    NotObject,
    #[error("Preset field '{0}' must be an array")]
    InvalidArray(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
struct CmakeMinimumRequired {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Default for CmakeMinimumRequired {
    fn default() -> Self {
        Self {
            major: 3,
            minor: 25,
            patch: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigurePreset {
    #[serde(skip_serializing_if = "Option::is_none")]
    inherits: Option<String>,
    name: String,
    #[serde(skip_serializing_if = "is_false")]
    hidden: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    binary_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    toolchain_file: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    cache_variables: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    environment: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vendor: Option<Vendor>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildPreset {
    name: String,
    configure_preset: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    configuration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vendor: Option<Vendor>,
}

#[derive(Debug, Clone, Serialize)]
struct Vendor {
    winch: VendorWinch,
}

impl Vendor {
    fn winch(prefix: &str) -> Self {
        Self {
            winch: VendorWinch {
                generated: true,
                config: prefix.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct VendorWinch {
    generated: bool,
    config: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CmakePresets {
    version: u32,
    cmake_minimum_required: CmakeMinimumRequired,
    configure_presets: Vec<ConfigurePreset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    build_presets: Vec<BuildPreset>,
}

struct GeneratedPresets {
    configure: Vec<ConfigurePreset>,
    build: Vec<BuildPreset>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

pub fn generate(
    delta: &EnvDelta,
    config: &Config,
    config_path: &Path,
    output: &Path,
) -> Result<(), GenerateError> {
    let prefix = preset_prefix(config_path);
    let generated = generate_presets(delta, config, &prefix);
    let existing = match std::fs::read_to_string(output) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };

    let content = match existing {
        Some(text) => merge(&text, &generated, &prefix)?,
        None => new_file(&generated)?,
    };

    write_atomic(output, &content)
}

fn generate_presets(delta: &EnvDelta, config: &Config, prefix: &str) -> GeneratedPresets {
    let environment = build_environment(delta);
    let vcpkg = vcpkg_settings(delta, config);

    let mut cache_variables = BTreeMap::new();
    if let Some((_, triplet)) = &vcpkg {
        cache_variables.insert("VCPKG_TARGET_TRIPLET".to_string(), triplet.clone());
    }

    let base_name = format!("{prefix}-base");
    let debug_name = format!("{prefix}-debug");
    let release_name = format!("{prefix}-release");

    let base = ConfigurePreset {
        inherits: None,
        name: base_name.clone(),
        hidden: true,
        binary_dir: None,
        toolchain_file: vcpkg.map(|(toolchain, _)| toolchain),
        cache_variables,
        environment,
        vendor: Some(Vendor::winch(prefix)),
    };

    GeneratedPresets {
        configure: vec![
            base,
            visible_preset(&base_name, &debug_name, "Debug", prefix),
            visible_preset(&base_name, &release_name, "Release", prefix),
        ],
        build: vec![
            build_preset(&debug_name, "Debug", prefix),
            build_preset(&release_name, "Release", prefix),
        ],
    }
}

fn visible_preset(base: &str, name: &str, build_type: &str, prefix: &str) -> ConfigurePreset {
    ConfigurePreset {
        inherits: Some(base.to_string()),
        name: name.to_string(),
        hidden: false,
        binary_dir: Some(format!("${{sourceDir}}/build/{name}")),
        toolchain_file: None,
        cache_variables: BTreeMap::from([("CMAKE_BUILD_TYPE".to_string(), build_type.to_string())]),
        environment: BTreeMap::new(),
        vendor: Some(Vendor::winch(prefix)),
    }
}

fn build_preset(name: &str, configuration: &str, prefix: &str) -> BuildPreset {
    BuildPreset {
        name: name.to_string(),
        configure_preset: name.to_string(),
        configuration: Some(configuration.to_string()),
        vendor: Some(Vendor::winch(prefix)),
    }
}

fn build_environment(delta: &EnvDelta) -> BTreeMap<String, String> {
    let mut environment = delta
        .set
        .iter()
        .map(|(key, values)| (key.clone(), env_to_string(values.iter())))
        .collect::<BTreeMap<_, _>>();

    let mut keys = Vec::new();
    for key in delta.prepend.keys().chain(delta.append.keys()) {
        if !keys
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(key))
        {
            keys.push(key.clone());
        }
    }

    for key in keys {
        let prepend = delta
            .prepend_values(&key)
            .map(|values| env_to_string(values.iter().rev()));
        let append = delta
            .append_values(&key)
            .map(|values| env_to_string(values.iter()));
        let parent = format!("$penv{{{key}}}");
        let value = match (prepend, append) {
            (Some(prepend), Some(append)) => format!("{prepend};{parent};{append}"),
            (Some(prepend), None) => format!("{prepend};{parent}"),
            (None, Some(append)) => format!("{parent};{append}"),
            (None, None) => continue,
        };
        environment.insert(key, value);
    }

    environment
}

fn vcpkg_settings(delta: &EnvDelta, config: &Config) -> Option<(String, String)> {
    let vcpkg = config.dev.as_ref()?.vcpkg.as_ref()?;
    if !vcpkg.enable {
        return None;
    }

    let root = delta.set_value("VCPKG_ROOT")?;
    let toolchain = Path::new(root)
        .join("scripts")
        .join("buildsystems")
        .join("vcpkg.cmake");
    Some((
        toolchain.to_string_lossy().into_owned(),
        vcpkg.triplet.clone(),
    ))
}

fn new_file(generated: &GeneratedPresets) -> Result<String, GenerateError> {
    let presets = CmakePresets {
        version: PRESETS_VERSION,
        cmake_minimum_required: CmakeMinimumRequired::default(),
        configure_presets: generated.configure.clone(),
        build_presets: generated.build.clone(),
    };
    Ok(serde_json::to_string_pretty(&presets)? + "\n")
}

fn merge(
    existing: &str,
    generated: &GeneratedPresets,
    prefix: &str,
) -> Result<String, GenerateError> {
    let mut root = serde_json::from_str::<Value>(existing)?;
    let object = root.as_object_mut().ok_or(GenerateError::NotObject)?;

    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .map(|version| version.max(u64::from(PRESETS_VERSION)))
        .unwrap_or(u64::from(PRESETS_VERSION));
    object.insert("version".to_string(), Value::from(version));

    let default_minimum = CmakeMinimumRequired::default();
    match minimum_required(object) {
        Some(minimum) if minimum >= default_minimum => {}
        _ => {
            object.insert(
                "cmakeMinimumRequired".to_string(),
                serde_json::to_value(default_minimum)?,
            );
        }
    }

    replace_winch_presets(object, "configurePresets", &generated.configure, prefix)?;
    replace_winch_presets(object, "buildPresets", &generated.build, prefix)?;

    Ok(serde_json::to_string_pretty(&root)? + "\n")
}

fn minimum_required(object: &Map<String, Value>) -> Option<CmakeMinimumRequired> {
    let value = object.get("cmakeMinimumRequired")?.as_object()?;
    Some(CmakeMinimumRequired {
        major: value.get("major")?.as_u64()?.try_into().ok()?,
        minor: value.get("minor")?.as_u64()?.try_into().ok()?,
        patch: value.get("patch")?.as_u64()?.try_into().ok()?,
    })
}

fn replace_winch_presets<T: Serialize>(
    object: &mut Map<String, Value>,
    field: &'static str,
    generated: &[T],
    prefix: &str,
) -> Result<(), GenerateError> {
    let entry = object
        .entry(field.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let array = entry
        .as_array_mut()
        .ok_or(GenerateError::InvalidArray(field))?;

    array.retain(|preset| !is_winch_generated(preset, prefix));
    for preset in generated {
        array.push(serde_json::to_value(preset)?);
    }
    Ok(())
}

fn is_winch_generated(preset: &Value, prefix: &str) -> bool {
    let Some(winch) = preset.get("vendor").and_then(|vendor| vendor.get("winch")) else {
        return false;
    };

    if !winch
        .get("generated")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return false;
    }

    match winch.get("config").and_then(Value::as_str) {
        Some(config) => config == prefix,
        None => prefix == "winch",
    }
}

fn write_atomic(path: &Path, content: &str) -> Result<(), GenerateError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = path.with_extension("tmp");
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(content.as_bytes())?;
    file.flush()?;
    drop(file);

    if let Err(error) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::EnvObj;
    use crate::project_config::EnvConfig;

    fn config() -> Config {
        Config {
            lib: Default::default(),
            toolchain: None,
            dev: None,
            env: EnvConfig::default(),
            pure: false,
            pure_keep: None,
        }
    }

    fn preset_names(value: &Value, field: &str) -> Vec<String> {
        value[field]
            .as_array()
            .unwrap()
            .iter()
            .map(|preset| preset["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn environment_handles_set_prepend_append() {
        let mut delta = EnvDelta::default();
        delta.set("JAVA_HOME", EnvObj::new("C:/jdk", "java"));
        delta.prepend("Path", EnvObj::new("C:/first", "msvc"));
        delta.prepend("Path", EnvObj::new("C:/second", "vcpkg"));
        delta.append("Path", EnvObj::new("C:/last", "user config"));

        let environment = build_environment(&delta);
        assert_eq!(environment["JAVA_HOME"], "C:/jdk");
        assert_eq!(
            environment["Path"],
            "C:/second;C:/first;$penv{Path};C:/last"
        );
    }

    #[test]
    fn new_file_contains_expected_presets() {
        let generated = generate_presets(&EnvDelta::default(), &config(), "winch");
        let text = new_file(&generated).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();

        assert_eq!(value["version"].as_u64(), Some(u64::from(PRESETS_VERSION)));
        assert_eq!(
            preset_names(&value, "configurePresets"),
            ["winch-base", "winch-debug", "winch-release"]
        );
        assert_eq!(
            preset_names(&value, "buildPresets"),
            ["winch-debug", "winch-release"]
        );
    }

    #[test]
    fn preset_names_follow_config_name() {
        let prefix = preset_prefix(Path::new("winch.dev.toml"));
        assert_eq!(prefix, "winch-dev");

        let generated = generate_presets(&EnvDelta::default(), &config(), &prefix);
        let text = new_file(&generated).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();

        assert_eq!(
            preset_names(&value, "configurePresets"),
            ["winch-dev-base", "winch-dev-debug", "winch-dev-release"]
        );
    }

    #[test]
    fn merge_keeps_user_presets_and_replaces_generated() {
        let existing = r#"{
            "version": 3,
            "configurePresets": [
                {"name": "mine"},
                {"name": "winch-old", "vendor": {"winch": {"generated": true}}}
            ],
            "buildPresets": [{"name": "mine", "configurePreset": "mine"}]
        }"#;
        let generated = generate_presets(&EnvDelta::default(), &config(), "winch");
        let merged = merge(existing, &generated, "winch").unwrap();
        let value: Value = serde_json::from_str(&merged).unwrap();

        let configure = preset_names(&value, "configurePresets");
        assert!(configure.contains(&"mine".to_string()));
        assert!(!configure.contains(&"winch-old".to_string()));
        assert!(configure.contains(&"winch-base".to_string()));

        let build = preset_names(&value, "buildPresets");
        assert!(build.contains(&"mine".to_string()));
        assert!(build.contains(&"winch-debug".to_string()));
    }

    #[test]
    fn merge_only_replaces_matching_config() {
        let existing = r#"{
            "version": 3,
            "configurePresets": [
                {"name": "winch-dev-base", "vendor": {"winch": {"generated": true, "config": "winch-dev"}}}
            ]
        }"#;
        let generated = generate_presets(&EnvDelta::default(), &config(), "winch");
        let merged = merge(existing, &generated, "winch").unwrap();
        let value: Value = serde_json::from_str(&merged).unwrap();

        let configure = preset_names(&value, "configurePresets");
        assert!(configure.contains(&"winch-dev-base".to_string()));
        assert!(configure.contains(&"winch-base".to_string()));
        assert_eq!(
            configure
                .iter()
                .filter(|name| *name == "winch-base")
                .count(),
            1
        );
    }

    #[test]
    fn merge_raises_outdated_cmake_minimum_required() {
        let existing = r#"{
            "version": 3,
            "cmakeMinimumRequired": {"major": 3, "minor": 20, "patch": 0},
            "configurePresets": []
        }"#;
        let generated = generate_presets(&EnvDelta::default(), &config(), "winch");
        let merged = merge(existing, &generated, "winch").unwrap();
        let value: Value = serde_json::from_str(&merged).unwrap();

        assert_eq!(value["cmakeMinimumRequired"]["major"], 3);
        assert_eq!(value["cmakeMinimumRequired"]["minor"], 25);
        assert_eq!(value["cmakeMinimumRequired"]["patch"], 0);
    }

    #[test]
    fn merge_keeps_newer_cmake_minimum_required() {
        let existing = r#"{
            "version": 3,
            "cmakeMinimumRequired": {"major": 4, "minor": 0, "patch": 0},
            "configurePresets": []
        }"#;
        let generated = generate_presets(&EnvDelta::default(), &config(), "winch");
        let merged = merge(existing, &generated, "winch").unwrap();
        let value: Value = serde_json::from_str(&merged).unwrap();

        assert_eq!(value["cmakeMinimumRequired"]["major"], 4);
        assert_eq!(value["cmakeMinimumRequired"]["minor"], 0);
        assert_eq!(value["cmakeMinimumRequired"]["patch"], 0);
    }

    #[test]
    fn merge_rejects_non_object() {
        let generated = generate_presets(&EnvDelta::default(), &config(), "winch");
        assert!(matches!(
            merge("[]", &generated, "winch"),
            Err(GenerateError::NotObject)
        ));
    }
}
