use crate::env::{EnvDelta, EnvObj, env_to_string};

fn split_values<'a>(envs: impl Iterator<Item = &'a EnvObj>) -> Vec<&'a str> {
    envs.flat_map(|env| env.value.split(';')).collect()
}

fn convert_windows_path_to_posix(path: &str) -> String {
    let path = path.replace("\\", "/");
    for drive in 'a'..='z' {
        if path.starts_with(&format!("{drive}:"))
            || path.starts_with(&format!("{}:", drive.to_ascii_uppercase()))
        {
            return format!("/{drive}{}", path.chars().skip(2).collect::<String>());
        }
    }
    path
}

fn convert_windows_to_posix(value: &str) -> String {
    value
        .split(';')
        .map(convert_windows_path_to_posix)
        .collect::<Vec<String>>()
        .join(":")
}

trait Escape {
    fn escape_bash(&self) -> String;
    fn escape_powershell(&self) -> String;
    fn escape_nu(&self) -> String;
}

impl Escape for str {
    fn escape_bash(&self) -> String {
        self.replace("\"", "\\\"").replace("$", "\\$")
    }

    fn escape_powershell(&self) -> String {
        self.replace("\"", "`\"").replace("$", "`$")
    }

    fn escape_nu(&self) -> String {
        self.replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("(", "\\(")
            .replace(")", "\\)")
    }
}

pub fn export_bash(env_delta: &EnvDelta) -> String {
    let mut bash = String::new();
    for (key, value) in env_delta.set.iter() {
        bash.push_str(&format!(
            "export {key}=\"{}\"\n",
            convert_windows_to_posix(&env_to_string(value.iter())).escape_bash()
        ));
    }
    for (key, value) in env_delta.prepend.iter() {
        if key.eq_ignore_ascii_case("Path") {
            bash.push_str(&format!(
                "export PATH=\"{}:$PATH\"\n",
                convert_windows_to_posix(&env_to_string(value.iter().rev())).escape_bash()
            ));
            continue;
        }
        bash.push_str(&format!(
            "export {key}=\"{}:${key}\"\n",
            convert_windows_to_posix(&env_to_string(value.iter().rev())).escape_bash()
        ));
    }
    for (key, value) in env_delta.append.iter() {
        if key.eq_ignore_ascii_case("Path") {
            bash.push_str(&format!(
                "export PATH=\"$PATH:{}\"\n",
                convert_windows_to_posix(&env_to_string(value.iter())).escape_bash()
            ));
            continue;
        }
        bash.push_str(&format!(
            "export {key}=\"${key}:{}\"\n",
            convert_windows_to_posix(&env_to_string(value.iter())).escape_bash()
        ));
    }
    bash
}

pub fn export_powershell(env_delta: &EnvDelta) -> String {
    let mut powershell = String::new();

    for (key, value) in env_delta.set.iter() {
        powershell.push_str(&format!(
            "${{env:{key}}} = \"{}\"\n",
            env_to_string(value.iter()).escape_powershell()
        ));
    }

    for (key, value) in env_delta.prepend.iter() {
        powershell.push_str(&format!(
            "${{env:{key}}} = \"{};${{env:{key}}}\"\n",
            env_to_string(value.iter().rev()).escape_powershell()
        ));
    }

    for (key, value) in env_delta.append.iter() {
        powershell.push_str(&format!(
            "${{env:{key}}} = \"${{env:{key}}};{}\"\n",
            env_to_string(value.iter()).escape_powershell()
        ));
    }

    powershell
}

pub fn export_nu(env_delta: &EnvDelta) -> String {
    let mut nu = String::new();
    for (key, value) in env_delta.set.iter() {
        let values = split_values(value.iter());
        if values.len() > 1 {
            nu.push_str(&format!("$env.\"{key}\" = {values:?}\n"));
        } else {
            nu.push_str(&format!(
                "$env.\"{key}\" = {:?}\n",
                values.first().copied().unwrap_or_default()
            ));
        }
    }

    for (key, value) in env_delta.prepend.iter() {
        if key.eq_ignore_ascii_case("Path") {
            let values = split_values(value.iter().rev());
            nu.push_str(&format!(
                "$env.\"{key}\" = {values:?} | append ($env | get -o \"{key}\")\n"
            ));
            continue;
        }

        let values = env_to_string(value.iter().rev());
        nu.push_str(&format!(
            "$env.\"{key}\" = $\"{};($env | get -o \"{key}\")\"\n",
            values.escape_nu()
        ));
    }

    for (key, value) in env_delta.append.iter() {
        if key.eq_ignore_ascii_case("Path") {
            let values = split_values(value.iter());
            nu.push_str(&format!(
                "$env.\"{key}\" = ($env | get -o \"{key}\") | append {values:?}\n"
            ));
            continue;
        }
        let values = env_to_string(value.iter());
        nu.push_str(&format!(
            "$env.\"{key}\" = $\"($env | get -o \"{key}\");{}\"\n",
            values.escape_nu()
        ));
    }
    nu
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_delta() -> EnvDelta {
        let mut delta = EnvDelta::default();
        delta.set("FLAGS", EnvObj::new("a;b", "test"));
        delta.prepend("Path", EnvObj::new("C:/first;C:/second", "test"));
        delta.prepend("Path", EnvObj::new("C:/third", "test"));
        delta.prepend("LIB", EnvObj::new("C:/libA", "test"));
        delta.prepend("LIB", EnvObj::new("C:/libB", "test"));
        delta.append("Path", EnvObj::new("C:/last1;C:/last2", "test"));
        delta
    }

    #[test]
    fn base_escape() {
        assert_eq!("\"$abc\"".escape_bash(), "\\\"\\$abc\\\"");
    }

    #[test]
    fn powershell_escape() {
        assert_eq!("\"${env:Path}\"".escape_powershell(), "`\"`${env:Path}`\"");
    }

    #[test]
    fn bash_prepend_order_and_path_conversion() {
        let output = export_bash(&sample_delta());
        assert!(output.contains("export PATH=\"/c/third:/c/first:/c/second:$PATH\""));
        assert!(output.contains("export LIB=\"/c/libB:/c/libA:$LIB\""));
        assert!(output.contains("export PATH=\"$PATH:/c/last1:/c/last2\""));
    }

    #[test]
    fn bash_treats_path_case_insensitively() {
        let mut delta = EnvDelta::default();
        delta.prepend("path", EnvObj::new("C:/pre", "test"));

        let output = export_bash(&delta);
        assert!(output.contains("export PATH=\"/c/pre:$PATH\""));
    }

    #[test]
    fn powershell_uses_semicolon_for_prepend() {
        let output = export_powershell(&sample_delta());
        assert!(output.contains("${env:Path} = \"C:/third;C:/first;C:/second;${env:Path}\""));
        assert!(output.contains("${env:LIB} = \"C:/libB;C:/libA;${env:LIB}\""));
    }

    #[test]
    fn nu_non_path_prepend_and_append_keep_order() {
        let mut delta = EnvDelta::default();
        delta.prepend("LIB", EnvObj::new("first", "test"));
        delta.prepend("LIB", EnvObj::new("second", "test"));
        delta.append("FLAGS", EnvObj::new("a", "test"));
        delta.append("FLAGS", EnvObj::new("b", "test"));

        let output = export_nu(&delta);
        assert!(output.contains("$env.\"LIB\" = $\"second;first;($env | get -o \"LIB\")\""));
        assert!(output.contains("$env.\"FLAGS\" = $\"($env | get -o \"FLAGS\");a;b\""));
    }

    #[test]
    fn nu_splits_all_semicolon_values() {
        let output = export_nu(&sample_delta());
        assert!(output.contains("$env.\"FLAGS\" = [\"a\", \"b\"]"));
        assert!(output.contains(
            "$env.\"Path\" = [\"C:/third\", \"C:/first\", \"C:/second\"] | append ($env | get -o \"Path\")"
        ));
        assert!(output.contains(
            "$env.\"Path\" = ($env | get -o \"Path\") | append [\"C:/last1\", \"C:/last2\"]"
        ));
    }
}
