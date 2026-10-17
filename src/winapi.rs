use std::path::{Path, PathBuf};

pub fn get_known_folder_path(folder_id: &windows::core::GUID) -> Option<PathBuf> {
    let result = unsafe {
        windows::Win32::UI::Shell::SHGetKnownFolderPath(
            folder_id,
            windows::Win32::UI::Shell::KNOWN_FOLDER_FLAG(0),
            None,
        )
    };

    match result {
        Ok(path_ptr) => {
            let path = unsafe { path_ptr.to_string().ok()? };
            unsafe {
                windows::Win32::System::Com::CoTaskMemFree(Some(path_ptr.0 as *mut _));
            }
            Some(PathBuf::from(path))
        }
        Err(_) => None,
    }
}

struct ComGuard {
    uninit: bool,
}

impl ComGuard {
    fn new() -> windows::core::Result<Self> {
        use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, S_FALSE, S_OK};
        use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

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
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

fn read_wide<F>(read: F) -> Option<String>
where
    F: FnOnce(&mut [u16]) -> windows::core::Result<()>,
{
    let mut buffer = vec![0u16; 32_768];
    read(&mut buffer).ok()?;

    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    if len == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..len]))
}

pub fn get_lnk_target<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{HSTRING, Interface, PCWSTR};

    let _com = ComGuard::new().ok()?;

    let path_h = HSTRING::from(path.as_ref().as_os_str());
    let link: IShellLinkW =
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.ok()?;
    let persist: IPersistFile = link.cast().ok()?;

    unsafe {
        persist.Load(PCWSTR(path_h.as_ptr()), STGM_READ).ok()?;
    }

    read_wide(|buffer| unsafe { link.GetPath(buffer, std::ptr::null_mut(), 0) }).map(PathBuf::from)
}
