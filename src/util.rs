use sha2::Digest;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::{BufReader, Read};
use std::path::Path;

const COVER: [&str; 2] = ["VCPKG_ROOT", "JAVA_HOME"];

pub fn env_add(env: &mut HashMap<String, String>, key: &str, value: &str) {
    if COVER.contains(&key) {
        env.insert(String::from(key), String::from(value));
    } else if env.contains_key(key) {
        let e = env.entry(key.to_string()).or_default();
        *e = format!("{value};{e}");
    } else {
        env.insert(key.to_string(), value.to_string());
    }
}

pub fn env_merge(env: &mut HashMap<String, String>, new_env: &HashMap<String, String>) {
    for (k, v) in new_env {
        env_add(env, k, v);
    }
}

pub fn sha256_file(path: &Path) -> io::Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0; 8192];

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    let result = hasher.finalize();

    Ok(hex::encode(result.as_slice()))
}
