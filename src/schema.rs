use crate::project_config::Config;
use serde_json::{Value, json};

pub fn generate() -> Result<String, serde_json::Error> {
    let schema = schemars::schema_for!(Config);
    let mut value = serde_json::to_value(schema)?;

    if let Some(object) = value.as_object_mut() {
        object.insert(
            "$id".to_string(),
            json!("https://raw.githubusercontent.com/yyxxryrx/winch/main/schema/winch.schema.json"),
        );
        object.insert("title".to_string(), json!("winch configuration"));
    }

    if let Some(pure_keep) = value
        .get_mut("properties")
        .and_then(|properties| properties.get_mut("pure_keep"))
        .and_then(Value::as_object_mut)
    {
        pure_keep.remove("default");
    }

    Ok(serde_json::to_string_pretty(&value)? + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_contains_config_fields() {
        let text = generate().unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();

        let properties = value["properties"].as_object().unwrap();
        for field in ["toolchain", "dev", "env", "pure", "pure_keep"] {
            assert!(properties.contains_key(field), "missing {field}");
        }
        assert!(value["$id"].is_string());
    }

    #[test]
    fn schema_exposes_msys2_types() {
        let text = generate().unwrap();
        assert!(text.contains("ucrt64"));
        assert!(text.contains("clangarm64"));
    }
}
