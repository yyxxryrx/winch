use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, S_FALSE, S_OK};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize, IPersistFile, STGM_READ,
};
use windows::Win32::UI::Shell::{IShellLinkW, KNOWN_FOLDER_FLAG, SHGetKnownFolderPath, ShellLink};
use windows::core::{GUID, HSTRING, Interface, PCWSTR};

pub fn get_known_folder_path(folder_id: &GUID) -> Option<PathBuf> {
    // 调用 API 获取路径
    let result = unsafe { SHGetKnownFolderPath(folder_id, KNOWN_FOLDER_FLAG(0), None) };

    match result {
        Ok(path_ptr) => {
            // 将 PWSTR 转换为 Rust String
            let path_str = unsafe { path_ptr.to_string().ok()? };

            // 释放 COM 分配的内存（非常重要，否则会导致内存泄漏）
            unsafe { CoTaskMemFree(Some(path_ptr.0 as *mut _)) };

            Some(PathBuf::from(path_str))
        }
        Err(..) => None,
    }
}

struct ComGuard {
    uninit: bool,
}

impl ComGuard {
    fn new() -> windows::core::Result<Self> {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

        if hr == S_OK {
            Ok(Self { uninit: true })
        } else if hr == S_FALSE || hr == RPC_E_CHANGED_MODE {
            Ok(Self { uninit: false })
        } else {
            Err(hr.into())
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninit {
            unsafe { CoUninitialize() }
        }
    }
}

fn read_wide<F>(read: F) -> Option<String>
where
    F: FnOnce(&mut [u16]) -> windows::core::Result<()>,
{
    let mut buf = vec![0u16; 32_768];

    if read(&mut buf).is_err() {
        return None;
    }

    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());

    if len == 0 {
        None
    } else {
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}

#[derive(Debug, Default)]
pub struct LnkInfo {
    pub target_path: Option<PathBuf>,
    pub arguments: Option<String>,
    pub working_directory: Option<PathBuf>,
}

pub fn get_lnk_info<P: AsRef<Path>>(path: P) -> windows::core::Result<LnkInfo> {
    let _com = ComGuard::new()?;

    let path_h = HSTRING::from(path.as_ref().as_os_str());

    let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)? };

    let persist: IPersistFile = link.cast()?;

    unsafe {
        persist.Load(PCWSTR(path_h.as_ptr()), STGM_READ)?;
    }

    let mut info = LnkInfo::default();

    info.target_path = read_wide(|buffer| unsafe { link.GetPath(buffer, std::ptr::null_mut(), 0) }).map(Into::into);

    info.arguments = read_wide(|p| unsafe { link.GetArguments(p) });

    info.working_directory = read_wide(|p| unsafe { link.GetWorkingDirectory(p) }).map(Into::into);

    Ok(info)
}
