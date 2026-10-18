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

fn find_key<V>(map: &BTreeMap<String, V>, key: &str) -> Option<String> {
    map.keys()
        .find(|existing| existing.eq_ignore_ascii_case(key))
        .cloned()
}

fn remove_key<V>(map: &mut BTreeMap<String, V>, key: &str) {
    if let Some(existing) = find_key(map, key) {
        map.remove(&existing);
    }
}

fn get_ci<'a, V>(map: &'a BTreeMap<String, V>, key: &str) -> Option<&'a V> {
    map.get(key).or_else(|| {
        map.iter()
            .find(|(existing, _)| existing.eq_ignore_ascii_case(key))
            .map(|(_, value)| value)
    })
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
    fn canonical_key(&self, key: &str) -> String {
        find_key(&self.set, key)
            .or_else(|| find_key(&self.prepend, key))
            .or_else(|| find_key(&self.append, key))
            .unwrap_or_else(|| key.to_string())
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<EnvObj>) {
        let key = key.into();
        let key = self.canonical_key(&key);
        remove_key(&mut self.prepend, &key);
        remove_key(&mut self.append, &key);
        self.set.insert(key, vec![value.into()]);
    }

    pub fn prepend(&mut self, key: impl Into<String>, value: impl Into<EnvObj>) {
        let key = key.into();
        let key = self.canonical_key(&key);
        let value = value.into();
        remove_key(&mut self.set, &key);
        self.prepend
            .entry(key)
            .and_modify(|existing| existing.push(value.clone()))
            .or_insert_with(|| vec![value]);
    }

    pub fn append(&mut self, key: impl Into<String>, value: impl Into<EnvObj>) {
        let key = key.into();
        let key = self.canonical_key(&key);
        let value = value.into();
        remove_key(&mut self.set, &key);
        self.append
            .entry(key)
            .and_modify(|existing| existing.push(value.clone()))
            .or_insert_with(|| vec![value]);
    }

    pub fn prepend_values(&self, key: &str) -> Option<&[EnvObj]> {
        get_ci(&self.prepend, key).map(Vec::as_slice)
    }

    pub fn append_values(&self, key: &str) -> Option<&[EnvObj]> {
        get_ci(&self.append, key).map(Vec::as_slice)
    }

    pub fn apply_to(&self, base: &mut HashMap<String, String>) {
        if self.pure {
            base.clear();
        }
        for (key, value) in &self.set {
            let value = env_to_string(value.iter());
            match base
                .iter_mut()
                .find(|(existing, _)| existing.eq_ignore_ascii_case(key))
            {
                Some((_, existing)) => *existing = value,
                None => {
                    base.insert(key.clone(), value);
                }
            }
        }
        for (key, value) in &self.prepend {
            let value = env_to_string(value.iter().rev());
            match base
                .iter_mut()
                .find(|(existing, _)| existing.eq_ignore_ascii_case(key))
            {
                Some((_, existing)) => *existing = format!("{value};{existing}"),
                None => {
                    base.insert(key.clone(), value);
                }
            }
        }
        for (key, value) in &self.append {
            let value = env_to_string(value.iter());
            match base
                .iter_mut()
                .find(|(existing, _)| existing.eq_ignore_ascii_case(key))
            {
                Some((_, existing)) => *existing = format!("{existing};{value}"),
                None => {
                    base.insert(key.clone(), value);
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
    fn keys_are_case_insensitive() {
        let mut delta = EnvDelta::default();
        delta.prepend("Path", EnvObj::new("first", "test"));
        delta.append("PATH", EnvObj::new("last", "test"));
        delta.prepend("path", EnvObj::new("second", "test"));

        assert_eq!(delta.prepend.len(), 1);
        assert_eq!(delta.append.len(), 1);
        assert!(delta.prepend.contains_key("Path"));
        assert!(delta.append.contains_key("Path"));
        assert_eq!(
            env_to_string(delta.prepend["Path"].iter().rev()),
            "second;first"
        );
        assert_eq!(env_to_string(delta.append["Path"].iter()), "last");
    }

    #[test]
    fn set_removes_other_sections_case_insensitively() {
        let mut delta = EnvDelta::default();
        delta.prepend("Path", EnvObj::new("pre", "test"));
        delta.append("PATH", EnvObj::new("post", "test"));
        delta.set("path", EnvObj::new("new", "test"));

        assert!(delta.prepend.is_empty());
        assert!(delta.append.is_empty());
        assert!(delta.set.contains_key("Path"));
    }

    #[test]
    fn apply_to_matches_base_keys_case_insensitively() {
        let mut base = HashMap::from([("Path".to_string(), "old".to_string())]);
        let mut delta = EnvDelta::default();
        delta.prepend("PATH", EnvObj::new("new", "test"));
        delta.append("path", EnvObj::new("tail", "test"));

        delta.apply_to(&mut base);

        assert_eq!(base.len(), 1);
        assert_eq!(base["Path"], "new;old;tail");
    }

    #[test]
    fn set_replaces_base_key_case_insensitively() {
        let mut base = HashMap::from([("Path".to_string(), "old".to_string())]);
        let mut delta = EnvDelta::default();
        delta.set("PATH", EnvObj::new("new", "test"));

        delta.apply_to(&mut base);

        assert_eq!(base.len(), 1);
        assert_eq!(base["Path"], "new");
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
