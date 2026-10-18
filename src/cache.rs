use crate::env::EnvDelta;
use log::debug;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::path::Path;

pub const FORMAT_VERSION: u32 = 4;
pub const CACHE_PATH: &str = ".winch/cache.toml";

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheFile {
    pub version: u32,
    pub hash: String,
    pub env: EnvDelta,
}

impl CacheFile {
    pub fn matches(&self, hash: &str) -> bool {
        self.version == FORMAT_VERSION && self.hash == hash
    }
}

pub fn hash_file(path: &Path) -> io::Result<String> {
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

    Ok(hex::encode(hasher.finalize()))
}

pub fn read(path: &Path) -> Option<CacheFile> {
    let text = std::fs::read_to_string(path).ok()?;
    match toml::from_str(&text) {
        Ok(cache) => Some(cache),
        Err(e) => {
            debug!("Ignoring invalid cache file: {e}");
            None
        }
    }
}

pub fn write(path: &Path, cache: &CacheFile) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let text = toml::to_string(cache).map_err(io::Error::other)?;
    let tmp = path.with_extension("tmp");

    let mut file = File::create(&tmp)?;
    file.write_all(text.as_bytes())?;
    file.flush()?;
    drop(file);

    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::{env_to_string, EnvObj};

    fn temp_path(name: &str) -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("winch-{}-{name}-{unique}", std::process::id()))
            .join("cache.toml")
    }

    fn sample_cache(hash: &str) -> CacheFile {
        let mut env = EnvDelta::default();
        env.set("JAVA_HOME", EnvObj::new("C:/jdk", "java"));
        env.prepend("Path", EnvObj::new("C:/jdk/bin", "java"));
        env.append("Path", EnvObj::new("C:/tools/bin", "java"));
        env.pure = true;
        CacheFile {
            version: FORMAT_VERSION,
            hash: hash.to_string(),
            env,
        }
    }

    #[test]
    fn cache_roundtrip() {
        let path = temp_path("roundtrip");
        let cache = sample_cache("abc123");
        write(&path, &cache).unwrap();

        let loaded = read(&path).unwrap();
        assert!(loaded.matches("abc123"));
        assert!(!loaded.matches("other"));
        assert_eq!(env_to_string(loaded.env.set["JAVA_HOME"].iter()), "C:/jdk");
        assert_eq!(env_to_string(loaded.env.prepend["Path"].iter().rev()), "C:/jdk/bin");
        assert_eq!(env_to_string(loaded.env.append["Path"].iter()), "C:/tools/bin");
        assert!(loaded.env.pure);

        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn invalid_cache_is_ignored() {
        let path = temp_path("invalid");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not valid toml {").unwrap();

        assert!(read(&path).is_none());

        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn wrong_format_version_does_not_match() {
        let mut cache = sample_cache("abc123");
        cache.version = FORMAT_VERSION + 1;
        assert!(!cache.matches("abc123"));
    }
}
