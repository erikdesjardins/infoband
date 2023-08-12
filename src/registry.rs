use crate::string::{encode_utf16_null_terminated, encode_utf16_null_terminated_as_bytes};
use std::ffi::OsStr;
use windows::core::{Result, HRESULT, PCWSTR};
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY,
    HKEY_CLASSES_ROOT, KEY_WOW64_64KEY, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
};

/// Represents an open HKEY, which can be used to set values in the registry
pub struct Key {
    // SAFETY: must never be exposed outside this module, or closed before the Key is dropped
    hkey: HKEY,
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: hkey is a valid open key handle, due to field invariant
        let err = unsafe { RegCloseKey(self.hkey) };
        if let Err(e) = err.ok() {
            log::warn!("Failed to close registry key: {}", e);
        }
    }
}

impl Key {
    pub fn delete_recursively(subkey: &str) -> Result<()> {
        let subkey = encode_utf16_null_terminated(subkey);

        // Delete tree first (which does not delete the key)...

        // SAFETY: subkey is a valid utf16 null-terminated string
        let result =
            unsafe { RegDeleteTreeW(HKEY_CLASSES_ROOT, PCWSTR::from_raw(subkey.as_ptr())).ok() };
        if let Err(e) = result {
            // Tolerate already deleted keys
            if e.code() != HRESULT::from(ERROR_FILE_NOT_FOUND) {
                return Err(e);
            }
        }

        // ...then delete the toplevel key.

        // SAFETY: subkey is a valid utf16 null-terminated string
        let result = unsafe {
            RegDeleteKeyExW(
                HKEY_CLASSES_ROOT,
                PCWSTR::from_raw(subkey.as_ptr()),
                KEY_WOW64_64KEY.0,
                0,
            )
            .ok()
        };
        if let Err(e) = result {
            // Tolerate already deleted keys
            if e.code() != HRESULT::from(ERROR_FILE_NOT_FOUND) {
                return Err(e);
            }
        }

        Ok(())
    }

    pub fn create(subkey: &str) -> Result<Self> {
        let subkey = encode_utf16_null_terminated(subkey);

        let mut hkey = HKEY::default();
        // SAFETY: subkey is a valid utf16 null-terminated string
        unsafe {
            RegCreateKeyExW(
                HKEY_CLASSES_ROOT,
                PCWSTR::from_raw(subkey.as_ptr()),
                0,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut hkey,
                None,
            )
            .ok()?
        };

        Ok(Self { hkey })
    }

    pub fn set_value(&self, name: Option<&str>, data: &OsStr) -> Result<()> {
        let name = name.map(encode_utf16_null_terminated);
        let data = encode_utf16_null_terminated_as_bytes(data);

        // SAFETY: hkey is a valid open key handle; name and data are valid null terminated utf16 string
        unsafe {
            RegSetValueExW(
                self.hkey,
                match name {
                    Some(name) => PCWSTR::from_raw(name.as_ptr()),
                    None => PCWSTR::null(),
                },
                0,
                REG_SZ,
                Some(data.as_slice()),
            )
            .ok()?;
        }

        Ok(())
    }
}
