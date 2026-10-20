use crate::env::{EnvDelta, env_to_string};
use crate::project_config::Config;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

const PRESETS_VERSION: u32 = 3;
const BASE_PRESET: &str = "winch-base";
const DEBUG_PRESET: &str = "winch-debug";
const RELEASE_PRESET: &str = "winch-release";

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

#[derive(Debug, Clone, Serialize)]
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
    fn winch() -> Self {
        Self {
            winch: VendorWinch { generated: true },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct VendorWinch {
    generated: bool,
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

pub fn generate(delta: &EnvDelta, config: &Config, output: &Path) -> Result<(), GenerateError> {
    let generated = generate_presets(delta, config);
    let existing = match std::fs::read_to_string(output) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };

    let content = match existing {
        Some(text) => merge(&text, &generated)?,
        None => new_file(&generated)?,
    };

    write_atomic(output, &content)
}

fn generate_presets(delta: &EnvDelta, config: &Config) -> GeneratedPresets {
    let environment = build_environment(delta);
    let vcpkg = vcpkg_settings(delta, config);

    let mut cache_variables = BTreeMap::new();
    if let Some((_, triplet)) = &vcpkg {
        cache_variables.insert("VCPKG_TARGET_TRIPLET".to_string(), triplet.clone());
    }

    let base = ConfigurePreset {
        inherits: None,
        name: BASE_PRESET.to_string(),
        hidden: true,
        binary_dir: None,
        toolchain_file: vcpkg.map(|(toolchain, _)| toolchain),
        cache_variables,
        environment,
        vendor: Some(Vendor::winch()),
    };

    GeneratedPresets {
        configure: vec![
            base,
            visible_preset(DEBUG_PRESET, "Debug"),
            visible_preset(RELEASE_PRESET, "Release"),
        ],
        build: vec![
            build_preset(DEBUG_PRESET, "Debug"),
            build_preset(RELEASE_PRESET, "Release"),
        ],
    }
}

fn visible_preset(name: &str, build_type: &str) -> ConfigurePreset {
    ConfigurePreset {
        inherits: Some(BASE_PRESET.to_string()),
        name: name.to_string(),
        hidden: false,
        binary_dir: Some(format!("${{sourceDir}}/build/{name}")),
        toolchain_file: None,
        cache_variables: BTreeMap::from([("CMAKE_BUILD_TYPE".to_string(), build_type.to_string())]),
        environment: BTreeMap::new(),
        vendor: Some(Vendor::winch()),
    }
}

fn build_preset(name: &str, configuration: &str) -> BuildPreset {
    BuildPreset {
        name: name.to_string(),
        configure_preset: name.to_string(),
        configuration: Some(configuration.to_string()),
        vendor: Some(Vendor::winch()),
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

fn merge(existing: &str, generated: &GeneratedPresets) -> Result<String, GenerateError> {
    let mut root = serde_json::from_str::<Value>(existing)?;
    let object = root.as_object_mut().ok_or(GenerateError::NotObject)?;

    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .map(|version| version.max(u64::from(PRESETS_VERSION)))
        .unwrap_or(u64::from(PRESETS_VERSION));
    object.insert("version".to_string(), Value::from(version));

    if !object.contains_key("cmakeMinimumRequired") {
        object.insert(
            "cmakeMinimumRequired".to_string(),
            serde_json::to_value(CmakeMinimumRequired::default())?,
        );
    }

    replace_winch_presets(object, "configurePresets", &generated.configure)?;
    replace_winch_presets(object, "buildPresets", &generated.build)?;

    Ok(serde_json::to_string_pretty(&root)? + "\n")
}

fn replace_winch_presets<T: Serialize>(
    object: &mut Map<String, Value>,
    field: &'static str,
    generated: &[T],
) -> Result<(), GenerateError> {
    let entry = object
        .entry(field.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let array = entry
        .as_array_mut()
        .ok_or(GenerateError::InvalidArray(field))?;

    array.retain(|preset| !is_winch_generated(preset));
    for preset in generated {
        array.push(serde_json::to_value(preset)?);
    }
    Ok(())
}

fn is_winch_generated(preset: &Value) -> bool {
    preset
        .get("vendor")
        .and_then(|vendor| vendor.get("winch"))
        .and_then(|winch| winch.get("generated"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
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
        let generated = generate_presets(&EnvDelta::default(), &config());
        let text = new_file(&generated).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();

        assert_eq!(value["version"].as_u64(), Some(u64::from(PRESETS_VERSION)));
        assert_eq!(
            preset_names(&value, "configurePresets"),
            [BASE_PRESET, DEBUG_PRESET, RELEASE_PRESET]
        );
        assert_eq!(
            preset_names(&value, "buildPresets"),
            [DEBUG_PRESET, RELEASE_PRESET]
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
        let generated = generate_presets(&EnvDelta::default(), &config());
        let merged = merge(existing, &generated).unwrap();
        let value: Value = serde_json::from_str(&merged).unwrap();

        let configure = preset_names(&value, "configurePresets");
        assert!(configure.contains(&"mine".to_string()));
        assert!(!configure.contains(&"winch-old".to_string()));
        assert!(configure.contains(&BASE_PRESET.to_string()));

        let build = preset_names(&value, "buildPresets");
        assert!(build.contains(&"mine".to_string()));
        assert!(build.contains(&DEBUG_PRESET.to_string()));
    }

    #[test]
    fn merge_rejects_non_object() {
        let generated = generate_presets(&EnvDelta::default(), &config());
        assert!(matches!(
            merge("[]", &generated),
            Err(GenerateError::NotObject)
        ));
    }
}
