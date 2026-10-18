use std::env;
use std::path::{Path, PathBuf};

fn valid(root: &Path) -> bool {
    root.join("usr").join("bin").join("msys-2.0.dll").is_file()
}

fn derive_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .take(6)
        .find(|candidate| valid(candidate))
        .map(Path::to_path_buf)
}

fn drive_letters() -> impl Iterator<Item = PathBuf> {
    (b'C'..=b'Z')
        .map(|letter| PathBuf::from(format!("{}:\\", letter as char)))
        .filter(|path| path.is_dir())
}

fn from_env() -> Option<PathBuf> {
    ["MSYS2_HOME", "MSYS_HOME"]
        .into_iter()
        .find_map(|var| env::var_os(var).and_then(|path| derive_root(Path::new(&path))))
}

fn from_registry() -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    let hives = [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE];
    let paths = [
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ];

    for hive in hives {
        for sub in paths {
            let Ok(root) = RegKey::predef(hive).open_subkey(sub) else {
                continue;
            };
            for name in root.enum_keys().flatten() {
                let Ok(key) = root.open_subkey(&name) else {
                    continue;
                };
                let display: String = key.get_value("DisplayName").unwrap_or_default();
                if !display.to_ascii_uppercase().contains("MSYS2") {
                    continue;
                }
                for value in ["InstallLocation", "InstallDir"] {
                    let Ok(dir) = key.get_value::<String, _>(value) else {
                        continue;
                    };
                    if let Some(root) = derive_root(Path::new(&dir)) {
                        return Some(root);
                    }
                }
                let icon: String = key.get_value("DisplayIcon").unwrap_or_default();
                let raw = icon.split(',').next().unwrap_or("").trim();
                if !raw.is_empty()
                    && let Some(root) = derive_root(Path::new(raw))
                {
                    return Some(root);
                }
            }
        }
    }
    None
}

fn from_common_paths() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for drive in drive_letters() {
        for sub in [
            "msys64",
            "msys32",
            "msys2",
            r"tools\msys64",
            r"tools\msys2",
            r"dev\msys64",
        ] {
            candidates.push(drive.join(sub));
        }
    }
    if let Some(program_files) = env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("msys64"));
    }
    if let Some(scoop) = env::var_os("SCOOP") {
        candidates.push(PathBuf::from(scoop).join(r"apps\msys2\current"));
    }
    if let Some(user_profile) = env::var_os("USERPROFILE") {
        candidates.push(PathBuf::from(user_profile).join(r"scoop\apps\msys2\current"));
    }
    candidates.into_iter().find(|candidate| valid(candidate))
}

fn from_drive_scan() -> Option<PathBuf> {
    const SKIP: [&str; 3] = ["$RECYCLE.BIN", "SYSTEM VOLUME INFORMATION", "WINDOWS"];

    for drive in drive_letters() {
        let Ok(top) = std::fs::read_dir(&drive) else {
            continue;
        };
        for level1 in top.flatten() {
            let path1 = level1.path();
            if !path1.is_dir() {
                continue;
            }
            let name = level1.file_name().to_string_lossy().to_ascii_uppercase();
            if SKIP.contains(&name.as_str()) {
                continue;
            }
            if valid(&path1) {
                return Some(path1);
            }
            let Ok(inner) = std::fs::read_dir(&path1) else {
                continue;
            };
            for level2 in inner.flatten() {
                let path2 = level2.path();
                if path2.is_dir() && valid(&path2) {
                    return Some(path2);
                }
            }
        }
    }
    None
}

fn find_msys2_lnk(path: &Path) -> Option<PathBuf> {
    let lnk = walkdir::WalkDir::new(path)
        .follow_links(false)
        .max_depth(5)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "lnk"))
        .map(|entry| entry.into_path())
        .find(|entry| entry.file_name().is_some_and(|name| name == "MSYS2.lnk"))?;

    crate::winapi::get_lnk_target(&lnk)?
        .parent()
        .map(Path::to_path_buf)
}

fn from_start_menu_lnk() -> Option<PathBuf> {
    use windows::Win32::UI::Shell::{FOLDERID_CommonStartMenu, FOLDERID_StartMenu};

    crate::winapi::get_known_folder_path(&FOLDERID_StartMenu)
        .and_then(|path| find_msys2_lnk(&path))
        .or_else(|| {
            crate::winapi::get_known_folder_path(&FOLDERID_CommonStartMenu)
                .and_then(|path| find_msys2_lnk(&path))
        })
}

pub(crate) fn is_valid_root(root: &Path) -> bool {
    valid(root)
}

pub fn find_msys2() -> Option<PathBuf> {
    find_msys2_with_hint(None)
}

pub fn find_msys2_with_hint(hint: Option<&Path>) -> Option<PathBuf> {
    let finders: [fn() -> Option<PathBuf>; 4] = [
        from_env,
        from_registry,
        from_common_paths,
        from_start_menu_lnk,
    ];
    finders
        .into_iter()
        .find_map(|finder| finder())
        .or_else(|| {
            hint.filter(|path| is_valid_root(path))
                .map(Path::to_path_buf)
        })
        .or_else(from_drive_scan)
}
