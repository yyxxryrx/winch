use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvObj {
    /// The environment variable value.
    pub value: String,
    /// The source of the environment variables, e.g., "java" or "msvc".
    pub source: String,
}

impl EnvObj {
    pub fn new(values: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            value: values.into(),
            source: source.into(),
        }
    }
}

pub(crate) fn env_to_string<'a>(envs: impl Iterator<Item = &'a EnvObj>) -> String {
    envs.map(|env| env.value.clone())
        .collect::<Vec<_>>()
        .join(";")
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvDelta {
    #[serde(default)]
    pub set: BTreeMap<String, Vec<EnvObj>>,
    #[serde(default)]
    pub prepend: BTreeMap<String, Vec<EnvObj>>,
    #[serde(default)]
    pub append: BTreeMap<String, Vec<EnvObj>>,
    #[serde(default)]
    pub pure: bool,
}

impl EnvDelta {
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<EnvObj>) {
        let key = key.into();
        self.prepend.remove(&key);
        self.append.remove(&key);
        self.set.insert(key, vec![value.into()]);
    }

    pub fn prepend(&mut self, key: impl Into<String>, value: impl Into<EnvObj>) {
        let key = key.into();
        let value = value.into();
        self.set.remove(&key);
        self.prepend
            .entry(key)
            .and_modify(|existing| existing.push(value.clone()))
            .or_insert_with(|| vec![value]);
    }

    pub fn append(&mut self, key: impl Into<String>, value: impl Into<EnvObj>) {
        let key = key.into();
        let value = value.into();
        self.set.remove(&key);
        self.append
            .entry(key)
            .and_modify(|existing| existing.push(value.clone()))
            .or_insert_with(|| vec![value]);
    }

    pub fn apply_to(&self, base: &mut HashMap<String, String>) {
        if self.pure {
            base.clear();
        }
        for (key, value) in &self.set {
            base.insert(key.clone(), env_to_string(value.iter()));
        }
        for (key, value) in &self.prepend {
            match base.get_mut(key) {
                Some(existing) => {
                    *existing = format!("{};{existing}", env_to_string(value.iter().rev()))
                }
                None => {
                    base.insert(key.clone(), env_to_string(value.iter().rev()));
                }
            }
        }
        for (key, value) in &self.append {
            match base.get_mut(key) {
                Some(existing) => *existing = format!("{existing};{}", env_to_string(value.iter())),
                None => {
                    base.insert(key.clone(), env_to_string(value.iter()));
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
        delta.prepend("Path", EnvObj::new("a", "test"));
        delta.prepend("Path", EnvObj::new("b", "test"));
        assert_eq!(env_to_string(delta.prepend["Path"].iter().rev()), "b;a");
    }

    #[test]
    fn append_accumulates_in_order() {
        let mut delta = EnvDelta::default();
        delta.append("Path", EnvObj::new("a", "test"));
        delta.append("Path", EnvObj::new("b", "test"));
        assert_eq!(env_to_string(delta.append["Path"].iter()), "a;b");
    }

    #[test]
    fn set_overrides_previous_prepend() {
        let mut delta = EnvDelta::default();
        delta.prepend("JAVA_HOME", EnvObj::new("old", "test"));
        delta.set("JAVA_HOME", EnvObj::new("new", "test"));
        assert!(delta.prepend.is_empty());
        assert_eq!(env_to_string(delta.set["JAVA_HOME"].iter()), "new");
    }

    #[test]
    fn set_overrides_previous_append() {
        let mut delta = EnvDelta::default();
        delta.append("JAVA_HOME", EnvObj::new("old", "test"));
        delta.set("JAVA_HOME", EnvObj::new("new", "test"));
        assert!(delta.append.is_empty());
        assert_eq!(env_to_string(delta.set["JAVA_HOME"].iter()), "new");
    }

    #[test]
    fn prepend_and_append_coexist() {
        let mut delta = EnvDelta::default();
        delta.prepend("Path", EnvObj::new("first", "test"));
        delta.append("Path", EnvObj::new("last", "test"));
        assert_eq!(env_to_string(delta.prepend["Path"].iter()), "first");
        assert_eq!(env_to_string(delta.append["Path"].iter()), "last");
    }

    #[test]
    fn pure_clears_base_environment() {
        let mut base = HashMap::from([
            ("Path".to_string(), "old".to_string()),
            ("KEEP".to_string(), "value".to_string()),
        ]);
        let mut delta = EnvDelta {
            pure: true,
            ..Default::default()
        };
        delta.set("FOO", EnvObj::new("bar", "test"));
        delta.prepend("Path", EnvObj::new("C:/tools", "test"));
        delta.apply_to(&mut base);

        assert_eq!(base.len(), 2);
        assert!(!base.contains_key("KEEP"));
        assert_eq!(base["FOO"], "bar");
        assert_eq!(base["Path"], "C:/tools");
    }

    #[test]
    fn apply_to_base_env() {
        let mut base = HashMap::from([
            ("Path".to_string(), "old".to_string()),
            ("JAVA_HOME".to_string(), "old-jdk".to_string()),
        ]);
        let mut delta = EnvDelta::default();
        delta.prepend("Path", EnvObj::new("new", "test"));
        delta.append("Path", EnvObj::new("tail", "test"));
        delta.set("JAVA_HOME", EnvObj::new("new-jdk", "test"));
        delta.set("VCPKG_ROOT", EnvObj::new("vcpkg", "test"));

        delta.apply_to(&mut base);

        assert_eq!(base["Path"], "new;old;tail");
        assert_eq!(base["JAVA_HOME"], "new-jdk");
        assert_eq!(base["VCPKG_ROOT"], "vcpkg");
    }
}
