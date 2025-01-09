use crate::constants::{IDT_Z_ORDER_TIMER, Z_ORDER_TIMER_COALESCE, Z_ORDER_TIMER_MS};
use std::cell::Cell;
use windows::core::{w, Error, Result, HRESULT};
use windows::Win32::Foundation::{ERROR_INVALID_WINDOW_HANDLE, ERROR_SUCCESS, HWND};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowLongW, KillTimer, SetCoalescableTimer, SetWindowPos, GWL_EXSTYLE,
    HWND_BOTTOM, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSENDCHANGING, SWP_NOSIZE, SWP_NOZORDER,
    WINDOW_EX_STYLE, WS_EX_TOPMOST,
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
    shell: Cell<HWND>,
    /// Whether our window is currently topmost.
    currently_topmost: Cell<Option<bool>>,
}

impl ZOrder {
    pub fn new() -> Result<Self> {
        let shell = get_shell_window()?;

        Ok(Self {
            shell: Cell::new(shell),
            // Initial state is "unknown".
            // (We always need to call it at least once, since it might not be all the way on the top or bottom.)
            currently_topmost: Cell::new(None),
        })
    }

    /// For some reason, the first call to SetWindowPos does nothing;
    /// so this fn must be called once before the first call to `update`.
    pub fn touch_window(&self, window: HWND) {
        if let Err(e) = self.touch_window_fallible(window) {
            log::error!("Touching window failed: {}", e);
        }
    }

    pub fn touch_window_fallible(&self, window: HWND) -> Result<()> {
        unsafe {
            SetWindowPos(
                window,
                None,
                0,
                0,
                0,
                0,
                SWP_NOZORDER | SWP_NOMOVE | SWP_NOSIZE | SWP_NOSENDCHANGING,
            )?
        };

        Ok(())
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

    fn queue_update_fallible(&self, window: HWND) -> Result<()> {
        // Debounce the update by killing the existing timer.
        self.kill_timer_fallible(window)?;

        if unsafe {
            SetCoalescableTimer(
                Some(window),
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

    fn kill_timer_fallible(&self, window: HWND) -> Result<()> {
        let res = unsafe { KillTimer(Some(window), IDT_Z_ORDER_TIMER.0) };

        match res {
            Ok(()) => Ok(()),
            Err(e) if e.code() == HRESULT::from(ERROR_SUCCESS) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Set our window's z-order to match the taskbar's.
    pub fn update(&self, window: HWND) {
        if let Err(e) = self.update_fallible(window) {
            log::error!("Z-order update failed: {}", e);
        }
    }

    fn update_fallible(&self, window: HWND) -> Result<()> {
        let is_shell_topmost = self.is_shell_topmost()?;

        self.set_z_order_to(window, is_shell_topmost)?;

        Ok(())
    }

    fn is_shell_topmost(&self) -> Result<bool> {
        match is_window_topmost(self.shell.get()) {
            Ok(is_topmost) => Ok(is_topmost),
            Err(e) if e.code() == HRESULT::from(ERROR_INVALID_WINDOW_HANDLE) => {
                log::warn!("Shell window handle is invalid (explorer crashed?); refetching");
                self.shell.set(get_shell_window()?);
                is_window_topmost(self.shell.get())
            }
            Err(e) => Err(e),
        }
    }

    fn set_z_order_to(&self, window: HWND, topmost: bool) -> Result<()> {
        log::debug!("Setting z-order to topmost={}", topmost);

        unsafe {
            SetWindowPos(
                window,
                Some(if topmost { HWND_TOPMOST } else { HWND_BOTTOM }),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOSENDCHANGING,
            )?
        };

        self.currently_topmost.set(Some(topmost));

        Ok(())
    }
}

fn get_shell_window() -> Result<HWND> {
    unsafe { FindWindowW(w!("Shell_TrayWnd"), None) }
}

fn is_window_topmost(handle: HWND) -> Result<bool> {
    let style = {
        let res = unsafe { GetWindowLongW(handle, GWL_EXSTYLE) };
        if res == 0 {
            return Err(Error::from_win32());
        }
        WINDOW_EX_STYLE(res as u32)
    };

    Ok(style.contains(WS_EX_TOPMOST))
}
