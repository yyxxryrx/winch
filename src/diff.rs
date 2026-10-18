use crate::env::EnvDelta;
use log::debug;
use std::collections::HashMap;

pub fn print(delta: &EnvDelta, before: &HashMap<String, String>, after: &HashMap<String, String>) {
    print!("{}", render(delta, before, after));
}

pub fn render(
    delta: &EnvDelta,
    before: &HashMap<String, String>,
    after: &HashMap<String, String>,
) -> String {
    let mut output = String::new();
    let mut changed = false;

    let mut keys = delta
        .set
        .keys()
        .chain(delta.prepend.keys())
        .chain(delta.append.keys())
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();

    for key in keys {
        let before_value = find_value(before, key);
        let after_value = find_value(after, key);
        if before_value == after_value {
            continue;
        }

        output.push_str(&format!("{key}\n"));
        if delta.set.contains_key(key) {
            if let Some(before_value) = before_value {
                output.push_str(&format!("  - {before_value}  (inherited)\n"));
            }
            output.push_str(&format!(
                "  + {}  ({})\n\n",
                after_value.unwrap_or("<unset>"),
                source_text(delta, key)
            ));
        } else {
            for (value, source) in contributions(delta, key) {
                output.push_str(&format!("  + {value}  ({source})\n"));
            }
            output.push('\n');
        }
        changed = true;
    }

    if delta.pure {
        let mut removed = before
            .keys()
            .filter(|key| find_value(after, key).is_none())
            .map(String::as_str)
            .collect::<Vec<_>>();
        removed.sort_unstable();
        if !removed.is_empty() {
            for key in &removed {
                debug!("pure mode removes {key}");
            }
            output.push_str(&format!(
                "pure mode: {} inherited variable(s) removed\n\n",
                removed.len()
            ));
            changed = true;
        }
    }

    if !changed {
        output.push_str("No environment changes\n");
    }

    output
}

fn find_value<'a>(env: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    env.get(key)
        .or_else(|| {
            env.iter()
                .find(|(existing, _)| existing.eq_ignore_ascii_case(key))
                .map(|(_, value)| value)
        })
        .map(String::as_str)
}

fn contributions<'a>(delta: &'a EnvDelta, key: &str) -> Vec<(&'a str, &'a str)> {
    let mut entries = Vec::new();
    if let Some(values) = delta.prepend.get(key) {
        for value in values.iter().rev() {
            for part in value.value.split(';') {
                entries.push((part, value.source.as_str()));
            }
        }
    }
    if let Some(values) = delta.append.get(key) {
        for value in values.iter() {
            for part in value.value.split(';') {
                entries.push((part, value.source.as_str()));
            }
        }
    }
    entries
}

fn source_text(delta: &EnvDelta, key: &str) -> String {
    let mut sources = Vec::new();
    if let Some(values) = delta.set.get(key) {
        sources.extend(values.iter().map(|value| value.source.as_str()));
    }
    if let Some(values) = delta.prepend.get(key) {
        sources.extend(values.iter().rev().map(|value| value.source.as_str()));
    }
    if let Some(values) = delta.append.get(key) {
        sources.extend(values.iter().map(|value| value.source.as_str()));
    }

    let mut seen = std::collections::HashSet::new();
    sources.retain(|source| seen.insert(*source));
    if sources.is_empty() {
        "unknown".to_string()
    } else {
        sources.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::EnvObj;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn renders_contributions_with_sources() {
        let mut delta = EnvDelta::default();
        delta.set("FOO", EnvObj::new("bar", "user config"));
        delta.prepend("Path", EnvObj::new("C:/first", "msvc"));
        delta.prepend("Path", EnvObj::new("C:/second;C:/third", "vcpkg"));
        delta.append("Path", EnvObj::new("C:/last", "user config"));

        let before = env(&[("Path", "C:/Windows"), ("UNCHANGED", "same")]);
        let after = env(&[
            ("Path", "C:/second;C:/third;C:/first;C:/Windows;C:/last"),
            ("FOO", "bar"),
            ("UNCHANGED", "same"),
        ]);

        let output = render(&delta, &before, &after);
        assert!(output.contains("FOO\n  + bar  (user config)\n"));
        assert!(output.contains(
            "Path\n  + C:/second  (vcpkg)\n  + C:/third  (vcpkg)\n  + C:/first  (msvc)\n  + C:/last  (user config)\n"
        ));
        assert!(!output.contains("UNCHANGED"));
    }

    #[test]
    fn renders_set_replacement() {
        let mut delta = EnvDelta::default();
        delta.set("FOO", EnvObj::new("new", "user config"));

        let before = env(&[("FOO", "old")]);
        let after = env(&[("FOO", "new")]);

        let output = render(&delta, &before, &after);
        assert!(output.contains("FOO\n  - old  (inherited)\n  + new  (user config)\n"));
    }

    #[test]
    fn renders_pure_removals_as_summary() {
        let mut delta = EnvDelta {
            pure: true,
            ..Default::default()
        };
        delta.set("FOO", EnvObj::new("bar", "user config"));

        let before = env(&[("KEEP", "value"), ("FOO", "old")]);
        let after = env(&[("FOO", "bar")]);

        let output = render(&delta, &before, &after);
        assert!(output.contains("FOO\n  - old  (inherited)\n  + bar  (user config)\n"));
        assert!(output.contains("pure mode: 1 inherited variable(s) removed"));
    }

    #[test]
    fn matches_keys_case_insensitively() {
        let mut delta = EnvDelta::default();
        delta.prepend("PATH", EnvObj::new("C:/pre", "user config"));

        let before = env(&[("Path", "C:/Windows")]);
        let after = env(&[("Path", "C:/pre;C:/Windows")]);

        let output = render(&delta, &before, &after);
        assert!(output.contains("PATH\n  + C:/pre  (user config)\n"));
    }

    #[test]
    fn reports_no_changes() {
        let before = env(&[("FOO", "bar")]);
        let delta = EnvDelta::default();
        assert_eq!(render(&delta, &before, &before), "No environment changes\n");
    }
}
