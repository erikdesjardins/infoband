use std::ffi::OsString;
use std::os::windows::prelude::OsStringExt;
use std::path::PathBuf;
use std::sync::OnceLock;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{GetLastError, HMODULE};
use windows::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
};

static DUMMY: u16 = 0;

static HANDLE: OnceLock<HMODULE> = OnceLock::new();

/// Get a handle to the current DLL.
pub fn get_handle() -> HMODULE {
    *HANDLE.get_or_init(|| {
        let mut handle = HMODULE::default();
        // SAFETY: DUMMY is a valid address; unchanged refcount is okay because none of this code can run after the DLL unloads
        let ok = unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                PCWSTR::from_raw(&DUMMY),
                &mut handle,
            )
        };
        if !ok.as_bool() {
            // SAFETY: I don't think this has any preconditions?
            let error = unsafe { GetLastError() };
            panic!("GetModuleHandleExW failed: {:?}", error);
        }
        assert!(!handle.is_invalid());
        handle
    })
}

/// Gets the path to the provided DLL module.
///
/// # Safety
///
/// `dll_module` must be a valid handle to the current DLL.
pub unsafe fn get_dll_path(dll_module: HMODULE) -> PathBuf {
    let mut raw_path = [0_u16; 4096];

    // SAFETY: dll_module is guaranteed to be valid due to function safety requirement
    let len = unsafe { GetModuleFileNameW(dll_module, &mut raw_path) };

    if len == 0 {
        // SAFETY: I don't think this has any preconditions?
        let error = unsafe { GetLastError() };
        panic!("GetModuleFileNameW failed: {:?}", error);
    }

    PathBuf::from(OsString::from_wide(&raw_path[..len as usize]))
}

/// Gets the path to the current DLL.
pub fn get_current_dll_path() -> PathBuf {
    // SAFETY: get_handle() is guaranteed to return a valid handle to the current DLL
    unsafe { get_dll_path(get_handle()) }
}
