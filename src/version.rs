use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum GetVersionError {
    #[error("IOError: {0}")]
    IOError(#[from] std::io::Error),
    #[error("NotSuccess: exit with {0}")]
    NotSuccess(i32),
    #[error("Not match")]
    NotMatch,
}

pub type GetVersionResult<T> = Result<T, GetVersionError>;

pub fn get_clang_version(path: &Path) -> GetVersionResult<semver::Version> {
    let output = std::process::Command::new(path).args(["-v"]).output()?;

    if !output.status.success() {
        return Err(GetVersionError::NotSuccess(output.status.code().unwrap()));
    }

    let out = String::from_utf8_lossy(&output.stderr);
    // dbg!(&out);
    if let Some(caps) = regex::regex!(r"clang version (\d+\.\d+\.\d+)").captures(&out) {
        return Ok(semver::Version::parse(&caps[1]).unwrap());
    }
    Err(GetVersionError::NotMatch)
}

pub fn get_gcc_version(path: &Path) -> GetVersionResult<semver::Version> {
    let output = std::process::Command::new(path).args(["-v"]).output()?;

    if !output.status.success() {
        return Err(GetVersionError::NotSuccess(output.status.code().unwrap()));
    }

    let out = String::from_utf8_lossy(&output.stderr);
    if let Some(caps) = regex::regex!(r"gcc version (\d+\.\d+\.\d+)").captures(&out) {
        return Ok(semver::Version::parse(&caps[1]).unwrap());
    }
    Err(GetVersionError::NotMatch)
}

pub fn get_jdk_version(path: &Path) -> GetVersionResult<semver::Version> {
    let output = std::process::Command::new(path)
        .args(["-version"])
        .output()?;

    if !output.status.success() {
        return Err(GetVersionError::NotSuccess(output.status.code().unwrap()));
    }

    let out = String::from_utf8_lossy(&output.stderr);
    if let Some(caps) = regex::regex!(r#"openjdk version "(.*?)""#).captures(&out) {
        if caps[1].starts_with("1.") {
            return Ok(semver::Version::parse(
                &caps[1]
                    .chars()
                    .skip(2)
                    .collect::<String>()
                    .replace("_", "."),
            )
            .unwrap());
        }
        return Ok(semver::Version::parse(&caps[1]).unwrap());
    }
    Err(GetVersionError::NotMatch)
}
