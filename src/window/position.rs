use crate::constants::MICROPHONE_WARNING_WIDTH;
use crate::utils::ScalingFactor;
use crate::window::position::listener::TrayListenerManager;
use std::cell::{Cell, RefCell};
use std::mem;
use windows::Win32::Foundation::{
    ERROR_EMPTY, ERROR_INVALID_WINDOW_HANDLE, ERROR_SUCCESS, HWND, RECT,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, ROLE_SYSTEM_PANE, ROLE_SYSTEM_PUSHBUTTON,
    TreeScope_Children, TreeScope_Descendants, UIA_LegacyIAccessibleRolePropertyId,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GWL_EXSTYLE, GetWindowLongW, HWND_BOTTOM, HWND_TOPMOST, SWP_NOMOVE,
    SWP_NOSENDCHANGING, SWP_NOSIZE, SetWindowPos, USER_DEFAULT_SCREEN_DPI, WINDOW_EX_STYLE,
    WS_EX_TOPMOST,
};
use windows::core::{Error, HRESULT, Result, w};

mod listener;

/// Manages the position and z-order of the window.
///
/// In order to properly handle fullscreen windows, we need to match the z-order of the Windows taskbar.
/// Naturally, there is no proper API for this, and the logic that the taskbar itself uses is hacky and has bugs.
/// Thankfully, we can mostly avoid this, by listening to the same messages that the taskbar does,
/// but not actually performing the same logic, just checking whether the taskbar put itself on top or not and doing the same thing.
///
/// Big thanks to RudeWindowFixer, which contains a reverse engineered description of the taskbar's logic:
/// https://github.com/dechamps/RudeWindowFixer
pub struct Position {
    automation: IUIAutomation,
    /// The shell window, displaying the Windows taskbar.
    shell: Cell<HWND>,
    /// System tray area.
    tray: RefCell<TrayListenerManager>,
    /// DPI scaling factor of the window.
    dpi: Cell<ScalingFactor>,
    /// Size and position of the taskbar on the primary monitor.
    taskbar: Cell<RECT>,
    /// Left edge of the system tray area.
    tray_left_edge: Cell<i32>,
    /// Size and position of the window.
    rect: Cell<RECT>,
    /// Whether our window is currently topmost.
    currently_topmost: Cell<Option<bool>>,
}

impl Position {
    pub fn new(window: HWND) -> Result<Self> {
        let automation: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };

        let (shell, tray) = get_shell_window_and_system_tray(&automation)?;

        let tray = TrayListenerManager::new(window, automation.clone(), tray)?;

        let dpi = unsafe { GetDpiForWindow(shell) };
        let dpi = ScalingFactor::from_ratio(dpi, USER_DEFAULT_SCREEN_DPI);

        // Window starts out not displayed, with size and position zero.
        let empty = RECT {
            top: 0,
            bottom: 0,
            left: 0,
            right: 0,
        };

        Ok(Self {
            automation,
            shell: Cell::new(shell),
            tray: RefCell::new(tray),
            dpi: Cell::new(dpi),
            taskbar: Cell::new(empty),
            tray_left_edge: Cell::new(0),
            rect: Cell::new(empty),
            // Initial state is "unknown".
            // (We always need to call it at least once, since it might not be all the way on the top or bottom.)
            currently_topmost: Cell::new(None),
        })
    }

    pub fn get(&self) -> (RECT, ScalingFactor) {
        (self.rect.get(), self.dpi.get())
    }

    pub fn set_dpi(&self, dpi: u32) -> ScalingFactor {
        let dpi = ScalingFactor::from_ratio(dpi, USER_DEFAULT_SCREEN_DPI);
        self.dpi.set(dpi);
        dpi
    }

    pub fn update_taskbar_position(&self) {
        match self.get_taskbar_position() {
            Ok(rect) => {
                self.taskbar.set(rect);
            }
            Err(e) => {
                log::error!("Update taskbar position failed, preserving old position: {e}");
            }
        }
    }

    fn get_taskbar_position(&self) -> Result<RECT> {
        let monitor = unsafe { MonitorFromWindow(self.shell.get(), MONITOR_DEFAULTTOPRIMARY) };

        // Get size of primary monitor
        let monitor_info = {
            let mut monitor_info = MONITORINFO {
                cbSize: mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            // SAFETY: lpmi is valid pointer to MONITORINFO
            unsafe { GetMonitorInfoW(monitor, &mut monitor_info).ok()? };
            monitor_info
        };

        // Taskbar is below the work area to the edges of the monitor
        Ok(RECT {
            top: monitor_info.rcWork.bottom,
            bottom: monitor_info.rcMonitor.bottom,
            left: monitor_info.rcMonitor.left,
            right: monitor_info.rcMonitor.right,
        })
    }

    pub fn update_tray_position(&self) {
        match self.get_left_edge_of_tray() {
            Ok(left_edge) => {
                self.tray_left_edge.set(left_edge);
            }
            Err(e) => {
                log::error!("Update tray left edge failed, preserving old position: {e}");
            }
        }
    }

    fn get_left_edge_of_tray(&self) -> Result<i32> {
        let first_tray_button =
            get_first_tray_button(&self.automation, self.tray.borrow().element())?;

        let rect = unsafe { first_tray_button.CurrentBoundingRectangle()? };

        Ok(rect.left)
    }

    #[must_use = "Window position must be applied after recomputing"]
    pub fn recompute(&self) -> (RECT, ScalingFactor) {
        match self.recompute_fallible() {
            Ok(rect) => {
                self.rect.set(rect);
            }
            Err(e) => {
                log::error!("Update window position failed, preserving old position: {e}");
            }
        }

        (self.rect.get(), self.dpi.get())
    }

    fn recompute_fallible(&self) -> Result<RECT> {
        let dpi = self.dpi.get();
        let taskbar = self.taskbar.get();
        let tray_left_edge = self.tray_left_edge.get();

        let midpoint = |a, b| a + (b - a) / 2;

        // Height is always the size of the taskbar
        let top = taskbar.top;
        let bottom = taskbar.bottom;
        // Right edge is adjacent to right edge of system tray
        let right = tray_left_edge;
        // Left edge positioned at the horizontal center of the display, with enough room for the mic warning
        let left =
            midpoint(taskbar.left, taskbar.right) - MICROPHONE_WARNING_WIDTH.scale_by(dpi) / 2;

        if top == bottom || left == right {
            return Err(Error::new(ERROR_EMPTY.into(), "Draw rectange is empty"));
        }

        Ok(RECT {
            top,
            bottom,
            left,
            right,
        })
    }

    /// Set our window's z-order to match the taskbar's.
    pub fn update_z_order(&self, window: HWND) {
        if let Err(e) = self.update_z_order_fallible(window) {
            log::error!("Z-order update failed: {e}");
        }
    }

    fn update_z_order_fallible(&self, window: HWND) -> Result<()> {
        let is_shell_topmost = self.is_shell_topmost()?;

        self.set_z_order_to(window, is_shell_topmost)?;

        Ok(())
    }

    fn is_shell_topmost(&self) -> Result<bool> {
        match is_window_topmost(self.shell.get()) {
            Ok(is_topmost) => Ok(is_topmost),
            Err(e) if e.code() == HRESULT::from(ERROR_INVALID_WINDOW_HANDLE) => {
                log::warn!("Shell window handle is invalid (explorer crashed?); refetching");
                let (shell, tray) = get_shell_window_and_system_tray(&self.automation)?;
                self.shell.set(shell);
                self.tray.borrow_mut().refresh_element(tray)?;

                is_window_topmost(self.shell.get())
            }
            Err(e) => Err(e),
        }
    }

    fn set_z_order_to(&self, window: HWND, topmost: bool) -> Result<()> {
        log::debug!("Setting z-order to topmost={topmost}");

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

fn get_shell_window_and_system_tray(
    automation: &IUIAutomation,
) -> Result<(HWND, IUIAutomationElement)> {
    let shell = unsafe { FindWindowW(w!("Shell_TrayWnd"), None)? };

    let role_is_pane = unsafe {
        automation.CreatePropertyCondition(
            UIA_LegacyIAccessibleRolePropertyId,
            &VARIANT::from(ROLE_SYSTEM_PANE as i32),
        )?
    };

    let tray = match unsafe {
        automation
            .ElementFromHandle(shell)?
            .FindFirst(TreeScope_Descendants, &role_is_pane)
    } {
        Ok(tray) => tray,
        Err(e) if e.code() == HRESULT::from(ERROR_SUCCESS) => {
            return Err(Error::new(
                ERROR_EMPTY.into(),
                "System tray not found in shell window",
            ));
        }
        Err(e) => return Err(e),
    };

    Ok((shell, tray))
}

fn get_first_tray_button(
    automation: &IUIAutomation,
    tray: &IUIAutomationElement,
) -> Result<IUIAutomationElement> {
    let role_is_pushbutton = unsafe {
        automation.CreatePropertyCondition(
            UIA_LegacyIAccessibleRolePropertyId,
            &VARIANT::from(ROLE_SYSTEM_PUSHBUTTON as i32),
        )?
    };

    match unsafe { tray.FindFirst(TreeScope_Children, &role_is_pushbutton) } {
        Ok(first_tray_button) => Ok(first_tray_button),
        Err(e) if e.code() == HRESULT::from(ERROR_SUCCESS) => Err(Error::new(
            ERROR_EMPTY.into(),
            "No tray buttons found in system tray",
        )),
        Err(e) => Err(e),
    }
}

fn is_window_topmost(handle: HWND) -> Result<bool> {
    let style = {
        let res = unsafe { GetWindowLongW(handle, GWL_EXSTYLE) };
        if res == 0 {
            return Err(Error::from_thread());
        }
        WINDOW_EX_STYLE(res as u32)
    };

    Ok(style.contains(WS_EX_TOPMOST))
}
