use std::path::Path;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum GetVersionError {
    #[error("IOError: {0}")]
    IOError(#[from] std::io::Error),
    #[error("NotSuccess: exit with {0}")]
    NotSuccess(i32),
    #[error("Output does not match")]
    NotMatch,
}

pub type GetVersionResult<T> = Result<T, GetVersionError>;

fn run(path: &Path, args: &[&str]) -> GetVersionResult<String> {
    let output = Command::new(path).args(args).output()?;
    if !output.status.success() {
        return Err(GetVersionError::NotSuccess(
            output.status.code().unwrap_or(-1),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stderr).into_owned())
}

fn capture(text: &str, pattern: &str) -> GetVersionResult<semver::Version> {
    let regex = regex::Regex::new(pattern).map_err(|_| GetVersionError::NotMatch)?;
    let captured = regex
        .captures(text)
        .and_then(|caps| caps.get(1))
        .ok_or(GetVersionError::NotMatch)?;
    semver::Version::parse(captured.as_str()).map_err(|_| GetVersionError::NotMatch)
}

pub fn get_clang_version(path: &Path) -> GetVersionResult<semver::Version> {
    let output = run(path, &["-v"])?;
    capture(&output, r"clang version (\d+\.\d+\.\d+)")
}

pub fn get_gcc_version(path: &Path) -> GetVersionResult<semver::Version> {
    let output = run(path, &["-v"])?;
    capture(&output, r"gcc version (\d+\.\d+\.\d+)")
}

pub fn get_jdk_version(path: &Path) -> GetVersionResult<semver::Version> {
    let output = run(path, &["-version"])?;
    let regex =
        regex::Regex::new(r#"openjdk version "(.*?)""#).map_err(|_| GetVersionError::NotMatch)?;
    let raw = regex
        .captures(&output)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .ok_or(GetVersionError::NotMatch)?;

    let normalized = match raw.strip_prefix("1.") {
        Some(rest) => rest.replace('_', "."),
        None => raw.to_string(),
    };
    semver::Version::parse(&normalized).map_err(|_| GetVersionError::NotMatch)
}
