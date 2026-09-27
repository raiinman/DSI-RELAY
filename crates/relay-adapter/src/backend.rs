use crate::sandbox::stable_appcontainer_api_available;
use serde::Serialize;
use std::ffi::CString;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{FreeLibrary, FARPROC, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};

pub const SANDBOX_BACKEND_MATRIX_VERSION: u32 = 1;
pub const MEASURED_WINDOWS_BUILDS: &[u32] = &[26200];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct WindowsVersion {
    pub major: u32,
    pub minor: u32,
    pub build: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SandboxBackendSelection {
    pub matrix_version: u32,
    pub windows: WindowsVersion,
    pub measured_build: bool,
    pub stable_appcontainer_api_available: bool,
    pub selected_backend: String,
    pub strong_untrusted_launch_enabled: bool,
    pub direct_worker_network: bool,
    pub brokered_egress_required: bool,
    pub reason: String,
}

#[repr(C)]
struct RtlOsVersionInfoW {
    size: u32,
    major: u32,
    minor: u32,
    build: u32,
    platform_id: u32,
    csd_version: [u16; 128],
}

type RtlGetVersionFn =
    unsafe extern "system" fn(*mut RtlOsVersionInfoW) -> i32;

struct ModuleHandle(HMODULE);

impl Drop for ModuleHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { FreeLibrary(self.0) };
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn load_ntdll() -> Option<ModuleHandle> {
    let name = wide("ntdll.dll");
    let module = unsafe {
        LoadLibraryExW(
            name.as_ptr(),
            null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    };
    (!module.is_null()).then_some(ModuleHandle(module))
}

pub fn current_windows_version() -> Option<WindowsVersion> {
    let module = load_ntdll()?;
    let name = CString::new("RtlGetVersion").ok()?;
    let proc: FARPROC =
        unsafe { GetProcAddress(module.0, name.as_ptr() as *const u8) };
    let raw = proc?;
    let function: RtlGetVersionFn = unsafe { std::mem::transmute(raw) };
    let mut info = RtlOsVersionInfoW {
        size: std::mem::size_of::<RtlOsVersionInfoW>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform_id: 0,
        csd_version: [0; 128],
    };
    let status = unsafe { function(&mut info) };
    if status < 0 {
        return None;
    }
    Some(WindowsVersion {
        major: info.major,
        minor: info.minor,
        build: info.build,
    })
}

pub fn select_backend_for(
    windows: WindowsVersion,
    stable_available: bool,
) -> SandboxBackendSelection {
    let measured = MEASURED_WINDOWS_BUILDS.contains(&windows.build);
    if stable_available && measured {
        return SandboxBackendSelection {
            matrix_version: SANDBOX_BACKEND_MATRIX_VERSION,
            windows,
            measured_build: true,
            stable_appcontainer_api_available: true,
            selected_backend: "stable_lpac_brokered_egress".to_string(),
            strong_untrusted_launch_enabled: true,
            direct_worker_network: false,
            brokered_egress_required: true,
            reason: "stable AppContainer/LPAC backend is qualified on this Windows build"
                .to_string(),
        };
    }

    SandboxBackendSelection {
        matrix_version: SANDBOX_BACKEND_MATRIX_VERSION,
        windows,
        measured_build: measured,
        stable_appcontainer_api_available: stable_available,
        selected_backend: "disabled".to_string(),
        strong_untrusted_launch_enabled: false,
        direct_worker_network: false,
        brokered_egress_required: true,
        reason: if !stable_available {
            "stable AppContainer APIs are unavailable".to_string()
        } else {
            "Windows build has not passed RELAY sandbox qualification"
                .to_string()
        },
    }
}

pub fn select_release_backend() -> Option<SandboxBackendSelection> {
    let windows = current_windows_version()?;
    Some(select_backend_for(
        windows,
        stable_appcontainer_api_available(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_build_selects_stable_backend() {
        let selected = select_backend_for(
            WindowsVersion {
                major: 10,
                minor: 0,
                build: 26200,
            },
            true,
        );
        assert_eq!(
            selected.selected_backend,
            "stable_lpac_brokered_egress"
        );
        assert!(selected.strong_untrusted_launch_enabled);
        assert!(!selected.direct_worker_network);
        assert!(selected.brokered_egress_required);
    }

    #[test]
    fn unmeasured_build_fails_closed() {
        let selected = select_backend_for(
            WindowsVersion {
                major: 10,
                minor: 0,
                build: 19045,
            },
            true,
        );
        assert_eq!(selected.selected_backend, "disabled");
        assert!(!selected.strong_untrusted_launch_enabled);
    }

    #[test]
    fn missing_stable_api_fails_closed() {
        let selected = select_backend_for(
            WindowsVersion {
                major: 10,
                minor: 0,
                build: 26200,
            },
            false,
        );
        assert_eq!(selected.selected_backend, "disabled");
        assert!(!selected.strong_untrusted_launch_enabled);
    }
}
