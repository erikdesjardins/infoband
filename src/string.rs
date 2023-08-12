use std::ffi::OsStr;
use std::os::windows::prelude::OsStrExt;

pub fn encode_utf16_null_terminated(str: &str) -> Vec<u16> {
    str.encode_utf16().chain([0]).collect()
}

pub fn encode_utf16_null_terminated_as_bytes(str: &OsStr) -> Vec<u8> {
    str.encode_wide()
        .flat_map(|c| c.to_le_bytes())
        .chain([0, 0])
        .collect()
}
