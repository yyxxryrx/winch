use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvDelta {
    #[serde(default)]
    pub set: BTreeMap<String, String>,
    #[serde(default)]
    pub prepend: BTreeMap<String, String>,
    #[serde(default)]
    pub append: BTreeMap<String, String>,
    #[serde(default)]
    pub pure: bool,
}

impl EnvDelta {
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.prepend.remove(&key);
        self.append.remove(&key);
        self.set.insert(key, value.into());
    }

    pub fn prepend(&mut self, key: impl Into<String>, value: impl AsRef<str>) {
        let key = key.into();
        let value = value.as_ref();
        self.set.remove(&key);
        self.prepend
            .entry(key)
            .and_modify(|existing| *existing = format!("{value};{existing}"))
            .or_insert_with(|| value.to_string());
    }

    pub fn append(&mut self, key: impl Into<String>, value: impl AsRef<str>) {
        let key = key.into();
        let value = value.as_ref();
        self.set.remove(&key);
        self.append
            .entry(key)
            .and_modify(|existing| *existing = format!("{existing};{value}"))
            .or_insert_with(|| value.to_string());
    }

    pub fn apply_to(&self, base: &mut HashMap<String, String>) {
        if self.pure {
            base.clear();
        }
        for (key, value) in &self.set {
            base.insert(key.clone(), value.clone());
        }
        for (key, value) in &self.prepend {
            match base.get_mut(key) {
                Some(existing) => *existing = format!("{value};{existing}"),
                None => {
                    base.insert(key.clone(), value.clone());
                }
            }
        }
        for (key, value) in &self.append {
            match base.get_mut(key) {
                Some(existing) => *existing = format!("{existing};{value}"),
                None => {
                    base.insert(key.clone(), value.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepend_accumulates_in_order() {
        let mut delta = EnvDelta::default();
        delta.prepend("Path", "a");
        delta.prepend("Path", "b");
        assert_eq!(delta.prepend["Path"], "b;a");
    }

    #[test]
    fn append_accumulates_in_order() {
        let mut delta = EnvDelta::default();
        delta.append("Path", "a");
        delta.append("Path", "b");
        assert_eq!(delta.append["Path"], "a;b");
    }

    #[test]
    fn set_overrides_previous_prepend() {
        let mut delta = EnvDelta::default();
        delta.prepend("JAVA_HOME", "old");
        delta.set("JAVA_HOME", "new");
        assert!(delta.prepend.is_empty());
        assert_eq!(delta.set["JAVA_HOME"], "new");
    }

    #[test]
    fn set_overrides_previous_append() {
        let mut delta = EnvDelta::default();
        delta.append("JAVA_HOME", "old");
        delta.set("JAVA_HOME", "new");
        assert!(delta.append.is_empty());
        assert_eq!(delta.set["JAVA_HOME"], "new");
    }

    #[test]
    fn prepend_and_append_coexist() {
        let mut delta = EnvDelta::default();
        delta.prepend("Path", "first");
        delta.append("Path", "last");
        assert_eq!(delta.prepend["Path"], "first");
        assert_eq!(delta.append["Path"], "last");
    }

    #[test]
    fn apply_to_base_env() {
        let mut base = HashMap::from([
            ("Path".to_string(), "old".to_string()),
            ("JAVA_HOME".to_string(), "old-jdk".to_string()),
        ]);
        let mut delta = EnvDelta::default();
        delta.prepend("Path", "new");
        delta.append("Path", "tail");
        delta.set("JAVA_HOME", "new-jdk");
        delta.set("VCPKG_ROOT", "vcpkg");

        delta.apply_to(&mut base);

        assert_eq!(base["Path"], "new;old;tail");
        assert_eq!(base["JAVA_HOME"], "new-jdk");
        assert_eq!(base["VCPKG_ROOT"], "vcpkg");
    }
}
