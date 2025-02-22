use std::mem;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::core::Result;

#[derive(Default)]
pub struct State;

impl State {
    pub fn fetch_percent(&self) -> Result<f64> {
        let mut mem_status = MEMORYSTATUSEX {
            dwLength: mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        // SAFETY: `mem_status` is a valid `MEMORYSTATUSEX`
        unsafe { GlobalMemoryStatusEx(&mut mem_status)? };

        let used = mem_status.ullTotalPhys - mem_status.ullAvailPhys;
        let total = mem_status.ullTotalPhys;
        let percent = (used * 100) as f64 / total as f64;

        Ok(percent)
    }
}
