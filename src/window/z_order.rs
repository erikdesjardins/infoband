use crate::constants::{IDT_Z_ORDER_TIMER, Z_ORDER_TIMER_COALESCE, Z_ORDER_TIMER_MS};
use std::cell::Cell;
use windows::core::{w, Error, Result, HRESULT};
use windows::Win32::Foundation::{ERROR_SUCCESS, HWND};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowLongW, KillTimer, SetCoalescableTimer, SetWindowPos, GWL_EXSTYLE,
    HWND_BOTTOM, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, WINDOW_EX_STYLE, WS_EX_TOPMOST,
};

/// Manages the z-order of the window.
///
/// In order to properly handle fullscreen windows, we need to match the z-order of the Windows taskbar.
/// Naturally, there is no proper API for this, and the logic that the taskbar itself uses is hacky and has bugs.
/// Thankfully, we can mostly avoid this, by listening to the same messages that the taskbar does,
/// but not actually performing the same logic, just checking whether the taskbar put itself on top or not and doing the same thing.
///
/// Big thanks to RudeWindowFixer, which contains a reverse engineered description of the taskbar's logic:
/// https://github.com/dechamps/RudeWindowFixer
pub struct ZOrder {
    /// The shell window, displaying the Windows taskbar.
    shell: HWND,
    /// Whether our window is currently topmost.
    currently_topmost: Cell<Option<bool>>,
}

impl ZOrder {
    pub fn new() -> Result<Self> {
        let shell = {
            let res = unsafe { FindWindowW(w!("Shell_TrayWnd"), None) };
            if res.0 == 0 {
                return Err(Error::from_win32());
            }
            res
        };

        Ok(Self {
            shell,
            // Initial state is "unknown".
            // (We always need to call it at least once, since it might not be all the way on the top or bottom.)
            currently_topmost: Cell::new(None),
        })
    }

    /// Should be called when a window state changes, so we can update our state to match the taskbar's.
    ///
    /// This is necessary because we receive shell hook events concurrently with the taskbar process,
    /// and our logic is much simpler, so we always end up winning the race and using its old z-order.
    pub fn queue_update(&self, window: HWND) {
        if let Err(e) = self.queue_update_fallible(window) {
            log::error!("Queuing z-order update failed: {}", e);
        }
    }

    pub fn queue_update_fallible(&self, window: HWND) -> Result<()> {
        // Debounce the update by killing the existing timer.
        self.kill_timer_fallible(window)?;

        if unsafe {
            SetCoalescableTimer(
                window,
                IDT_Z_ORDER_TIMER.0,
                Z_ORDER_TIMER_MS,
                None,
                Z_ORDER_TIMER_COALESCE,
            )
        } == 0
        {
            return Err(Error::from_win32());
        }

        Ok(())
    }

    pub fn kill_timer(&self, window: HWND) {
        if let Err(e) = self.kill_timer_fallible(window) {
            log::error!("Killing z-order update timer failed: {}", e);
        }
    }

    pub fn kill_timer_fallible(&self, window: HWND) -> Result<()> {
        let res = unsafe { KillTimer(window, IDT_Z_ORDER_TIMER.0) };

        match res {
            Ok(()) => Ok(()),
            Err(e) if e.code() == HRESULT::from(ERROR_SUCCESS) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// For some reason, SetWindowPos doesn't seem to have any effect on the first call.
    /// Get that out of the way here.
    pub fn touch_window(&self, window: HWND) {
        if let Err(e) = self.touch_window_fallible(window) {
            log::error!("Touching window failed: {}", e);
        }
    }

    pub fn touch_window_fallible(&self, window: HWND) -> Result<()> {
        unsafe { SetWindowPos(window, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE)? };

        Ok(())
    }

    /// Set our window's z-order to match the taskbar's.
    pub fn update(&self, window: HWND) {
        if let Err(e) = self.update_fallible(window) {
            log::error!("Z-order update failed: {}", e);
        }
    }

    pub fn update_fallible(&self, window: HWND) -> Result<()> {
        let style = {
            let res = unsafe { GetWindowLongW(self.shell, GWL_EXSTYLE) };
            if res == 0 {
                return Err(Error::from_win32());
            }
            WINDOW_EX_STYLE(res as u32)
        };

        let shell_is_currently_topmost = style.contains(WS_EX_TOPMOST);

        if Some(shell_is_currently_topmost) != self.currently_topmost.get() {
            log::debug!(
                "Setting our z-order to topmost={}",
                shell_is_currently_topmost
            );

            unsafe {
                SetWindowPos(
                    window,
                    if shell_is_currently_topmost {
                        HWND_TOPMOST
                    } else {
                        HWND_BOTTOM
                    },
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE,
                )?
            };

            self.currently_topmost.set(Some(shell_is_currently_topmost));
        }

        Ok(())
    }
}
