use std::cell::Cell;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::GetSystemTimes;
use windows::core::Result;

#[derive(Default)]
pub struct State {
    prev_times: Cell<Option<(u64, u64, u64)>>,
}

impl State {
    pub fn fetch_percent(&self) -> Result<f64> {
        let mut idle = FILETIME::default();
        let mut kernel_plus_idle = FILETIME::default();
        let mut user = FILETIME::default();
        // SAFETY: all pointers are to valid `FILETIME`s
        unsafe {
            GetSystemTimes(
                Some(&mut idle),
                Some(&mut kernel_plus_idle),
                Some(&mut user),
            )?
        };

        let to_100ns_intervals = |filetime: FILETIME| {
            u64::from(filetime.dwHighDateTime) << 32 | u64::from(filetime.dwLowDateTime)
        };

        let idle = to_100ns_intervals(idle);
        let kernel_plus_idle = to_100ns_intervals(kernel_plus_idle);
        let user = to_100ns_intervals(user);

        // On first sample, just store the current times and return zero.
        let percent = match self.prev_times.get() {
            Some((prev_idle, prev_kernel_plus_idle, prev_user)) => {
                let idle_delta = idle.wrapping_sub(prev_idle);
                let kernel_plus_idle_delta = kernel_plus_idle.wrapping_sub(prev_kernel_plus_idle);
                let user_delta = user.wrapping_sub(prev_user);

                let time_delta = kernel_plus_idle_delta + user_delta;
                let active_delta = time_delta - idle_delta;

                (active_delta * 100) as f64 / (time_delta as f64)
            }
            None => 0.0,
        };

        self.prev_times.set(Some((idle, kernel_plus_idle, user)));

        Ok(percent)
    }
}
