use std::sync::OnceLock;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

static HANDLE: OnceLock<HMODULE> = OnceLock::new();

/// Get a handle to the current exe.
pub fn get_handle() -> HMODULE {
    *HANDLE.get_or_init(|| {
        // SAFETY: no safety requirements when passing null
        let result = unsafe { GetModuleHandleW(None) };
        match result {
            Ok(handle) => {
                assert!(!handle.is_invalid());
                handle
            }
            Err(e) => panic!("GetModuleHandleW failed: {}", e),
        }
    })
}
