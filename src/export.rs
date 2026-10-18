use crate::env::{env_to_string, EnvDelta};

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

pub fn export_bash(env_delta: &EnvDelta) -> String {
    let mut bash = String::new();
    for (key, value) in env_delta.set.iter() {
        bash.push_str(&format!(
            "export {key}=\"{}\"\n",
            convert_windows_to_posix(&env_to_string(value.iter()))
        ));
    }
    for (key, value) in env_delta.prepend.iter() {
        if key == "Path" {
            bash.push_str(&format!(
                "export PATH=\"{}:$PATH\"\n",
                convert_windows_to_posix(&env_to_string(value.iter()))
            ));
            continue;
        }
        bash.push_str(&format!(
            "export {key}=\"{}:${key}\"\n",
            convert_windows_to_posix(&env_to_string(value.iter()))
        ));
    }
    for (key, value) in env_delta.append.iter() {
        if key == "Path" {
            bash.push_str(&format!(
                "export PATH=\"$PATH:{}\"\n",
                convert_windows_to_posix(&env_to_string(value.iter()))
            ));
            continue;
        }
        bash.push_str(&format!(
            "export {key}=\"${key}:{}\"\n",
            convert_windows_to_posix(&env_to_string(value.iter()))
        ));
    }
    bash
}

pub fn export_powershell(env_delta: &EnvDelta) -> String {
    let mut powershell = String::new();

    for (key, value) in env_delta.set.iter() {
        powershell.push_str(&format!("${{env:{key}}} = \"{}\"\n", env_to_string(value.iter())));
    }

    for (key, value) in env_delta.prepend.iter() {
        powershell.push_str(&format!("${{env:{key}}} = \"{}:${{env:{key}}}\"\n", env_to_string(value.iter())));
    }

    for (key, value) in env_delta.append.iter() {
        powershell.push_str(&format!("${{env:{key}}} = \"${{env:{key}}};{}\"\n", env_to_string(value.iter())));
    }

    powershell
}

pub fn export_nu(env_delta: &EnvDelta) -> String {
    let mut nu = String::new();
    for (key, value) in env_delta.set.iter() {
        if value.len() > 1 {
            nu.push_str(&format!(
                "$env.\"{key}\" = {:?}\n",
                value.into_iter().map(|v| v.value.clone()).collect::<Vec<_>>()
            ));
        } else {
            nu.push_str(&format!("$env.\"{key}\" = {:?}\n", value.first().unwrap().value));
        }
    }

    for (key, value) in env_delta.prepend.iter() {
        nu.push_str(&format!(
            "$env.\"{key}\" = {:?} | append ($env | get -o \"{key}\")\n",
            value.into_iter().map(|v| v.value.clone()).collect::<Vec<_>>()
        ));
    }

    for (key, value) in env_delta.append.iter() {
        nu.push_str(&format!(
            "$env.\"{key}\" = ($env | get -o \"{key}\") | append {:?}\n",
            value.into_iter().map(|v| v.value.clone()).collect::<Vec<_>>()
        ));
    }
    nu
}
