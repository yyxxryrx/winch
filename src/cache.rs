use crate::env::EnvDelta;
use log::debug;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const FORMAT_VERSION: u32 = 8;
pub const MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;

pub fn path_for(config_path: &Path) -> PathBuf {
    let identity = config_path
        .canonicalize()
        .unwrap_or_else(|_| config_path.to_path_buf());
    let mut hasher = sha2::Sha256::new();
    hasher.update(identity.to_string_lossy().to_ascii_lowercase().as_bytes());
    let hash = hex::encode(hasher.finalize());

    Path::new(".winch")
        .join("cache")
        .join(format!("{}.toml", &hash[..16]))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheFile {
    pub version: u32,
    pub hash: String,
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub msys2_root: Option<PathBuf>,
    pub env: EnvDelta,
}

impl CacheFile {
    pub fn matches(&self, hash: &str, fingerprint: &str, now: u64) -> bool {
        self.version == FORMAT_VERSION
            && self.hash == hash
            && self.fingerprint == fingerprint
            && now.saturating_sub(self.created_at) <= MAX_AGE_SECS
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
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
    use crate::env::{EnvObj, env_to_string};

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
        env.prepend("Path", EnvObj::new("C:/tools/bin", "vcpkg"));
        env.append("Path", EnvObj::new("C:/tools/bin", "vcpkg"));
        env.pure = true;
        env.keep = vec!["SystemRoot".to_string(), "TEMP".to_string()];
        CacheFile {
            version: FORMAT_VERSION,
            hash: hash.to_string(),
            fingerprint: "fingerprint".to_string(),
            created_at: 100,
            msys2_root: Some(PathBuf::from("C:/msys64")),
            env,
        }
    }

    #[test]
    fn cache_roundtrip() {
        let path = temp_path("roundtrip");
        let cache = sample_cache("abc123");
        write(&path, &cache).unwrap();

        let loaded = read(&path).unwrap();
        assert!(loaded.matches("abc123", "fingerprint", 100));
        assert!(!loaded.matches("other", "fingerprint", 100));
        assert!(!loaded.matches("abc123", "other", 100));
        assert_eq!(loaded.fingerprint, "fingerprint");
        assert_eq!(loaded.created_at, 100);
        assert_eq!(loaded.msys2_root.as_deref(), Some(Path::new("C:/msys64")));
        assert_eq!(env_to_string(loaded.env.set["JAVA_HOME"].iter()), "C:/jdk");
        assert_eq!(
            env_to_string(loaded.env.prepend["Path"].iter().rev()),
            "C:/tools/bin;C:/jdk/bin"
        );
        assert_eq!(loaded.env.prepend["Path"][0].source, "java");
        assert_eq!(loaded.env.prepend["Path"][1].source, "vcpkg");
        assert_eq!(
            env_to_string(loaded.env.append["Path"].iter()),
            "C:/tools/bin"
        );
        assert!(loaded.env.pure);
        assert_eq!(loaded.env.keep, ["SystemRoot", "TEMP"]);

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
    fn cache_paths_are_config_specific() {
        let first = path_for(Path::new("winch.toml"));
        let second = path_for(Path::new("winch.dev.toml"));

        assert_eq!(first, path_for(Path::new("winch.toml")));
        assert_ne!(first, second);
    }

    #[test]
    fn wrong_format_version_does_not_match() {
        let mut cache = sample_cache("abc123");
        cache.version = FORMAT_VERSION + 1;
        assert!(!cache.matches("abc123", "fingerprint", 100));
    }

    #[test]
    fn expired_cache_does_not_match() {
        let cache = sample_cache("abc123");
        assert!(cache.matches("abc123", "fingerprint", 100 + MAX_AGE_SECS));
        assert!(!cache.matches("abc123", "fingerprint", 101 + MAX_AGE_SECS));
    }
}
