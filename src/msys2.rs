use std::env;
use std::path::{Path, PathBuf};

/// 校验标志：MSYS2 根目录下必有 usr\bin\msys-2.0.dll
const MARKER: &str = r"usr\bin\msys-2.0.dll";

fn valid(root: &Path) -> bool {
    root.join(MARKER).is_file()
}

/// 给任意路径（可能是根目录、也可能是根下的 exe），向上找有效的 MSYS2 根
fn derive_root(p: &Path) -> Option<PathBuf> {
    let mut cur = Some(p.to_path_buf());
    for _ in 0..5 {
        if let Some(c) = cur {
            if valid(&c) {
                return Some(c);
            }
            cur = c.parent().map(|x| x.to_path_buf());
        } else {
            break;
        }
    }
    None
}

fn drive_letters() -> impl Iterator<Item = PathBuf> {
    (b'C'..=b'Z')
        .map(|l| PathBuf::from(format!("{}:\\", l as char)))
        .filter(|p| p.is_dir())
}

/// 第 1 层：环境变量 MSYS2_HOME / MSYS_HOME
fn from_env() -> Option<PathBuf> {
    for var in ["MSYS2_HOME", "MSYS_HOME"] {
        if let Some(p) = env::var_os(var)
            && let Some(r) = derive_root(&PathBuf::from(p))
        {
            return Some(r);
        }
    }
    None
}

/// 第 2 层：注册表卸载信息（HKCU + HKLM，含 WOW6432Node）
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
                // InstallLocation / InstallDir 通常就是根目录
                for val in ["InstallLocation", "InstallDir"] {
                    let dir: String = match key.get_value(val) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    if let Some(p) = derive_root(Path::new(&dir)) {
                        return Some(p);
                    }
                }
                // 兜底：DisplayIcon 形如 "C:\msys64\uninstall.exe,0"
                let icon: String = key.get_value("DisplayIcon").unwrap_or_default();
                let raw = icon.split(',').next().unwrap_or("").trim();
                if !raw.is_empty()
                    && let Some(p) = derive_root(Path::new(raw))
                {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// 第 3 层：常见默认路径（官方默认 / Chocolatey / Scoop / Program Files）
fn from_common_paths() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
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
    if let Some(pf) = env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(pf).join("msys64"));
    }
    if let Some(scoop) = env::var_os("SCOOP") {
        candidates.push(PathBuf::from(scoop).join(r"apps\msys2\current"));
    }
    if let Some(up) = env::var_os("USERPROFILE") {
        candidates.push(PathBuf::from(up).join(r"scoop\apps\msys2\current"));
    }
    candidates.into_iter().find(|c| valid(c))
}

/// 第 4 层（兜底）：扫描每个盘的前两级目录找 msys-2.0.dll
/// 覆盖 C:\msys64、C:\tools\msys64、D:\dev\msys64 这类布局，毫秒级完成
fn from_drive_scan() -> Option<PathBuf> {
    const SKIP: [&str; 3] = ["$RECYCLE.BIN", "SYSTEM VOLUME INFORMATION", "WINDOWS"];
    for drive in drive_letters() {
        let Ok(top) = std::fs::read_dir(&drive) else {
            continue;
        };
        for lvl1 in top.flatten() {
            let p1 = lvl1.path();
            if !p1.is_dir() {
                continue;
            }
            let name = lvl1.file_name().to_string_lossy().to_ascii_uppercase();
            if SKIP.contains(&name.as_str()) {
                continue;
            }
            if valid(&p1) {
                return Some(p1);
            }
            let Ok(inner) = std::fs::read_dir(&p1) else {
                continue;
            };
            for lvl2 in inner.flatten() {
                let p2 = lvl2.path();
                if p2.is_dir() && valid(&p2) {
                    return Some(p2);
                }
            }
        }
    }
    None
}

fn walk_dir(path: &Path) -> Option<PathBuf> {
    let msys2_lnk = walkdir::WalkDir::new(&path)
        .follow_links(false)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "lnk"))
        .map(|e| e.into_path())
        .find(|e| e.file_name().is_some_and(|name| name == "MSYS2.lnk"));
    if let Some(msys2_lnk) = msys2_lnk
        && let Ok(lnk) = crate::winapi::get_lnk_info(&msys2_lnk)
    {
        lnk.target_path
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    } else {
        None
    }
}

fn from_start_menu_lnk() -> Option<PathBuf> {
    let start_menu =
        crate::winapi::get_known_folder_path(&windows::Win32::UI::Shell::FOLDERID_StartMenu);
    let common_menu =
        crate::winapi::get_known_folder_path(&windows::Win32::UI::Shell::FOLDERID_CommonStartMenu);
    start_menu
        .and_then(|p| walk_dir(&p))
        .or_else(|| common_menu.and_then(|p| walk_dir(&p)))
}

pub fn find_msys2() -> Option<PathBuf> {
    from_env()
        .or_else(from_registry)
        .or_else(from_common_paths)
        .or_else(from_start_menu_lnk)
        .or_else(from_drive_scan)
}
