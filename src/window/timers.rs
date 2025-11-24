use crate::constants::{
    FETCH_AND_REDRAW_TIMER_COALESCE, FETCH_TIMER_MS, IDT_FETCH_AND_REDRAW_TIMER,
    IDT_MIC_STATE_TIMER, IDT_TRAY_POSITION_TIMER, IDT_Z_ORDER_TIMER, MIC_STATE_TIMER_COALESCE,
    MIC_STATE_TIMER_MS, TRAY_POSITION_TIMER_COALESCE, TRAY_POSITION_TIMER_MS,
    Z_ORDER_TIMER_COALESCE, Z_ORDER_TIMER_MS,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, SetCoalescableTimer};
use windows::core::{Error, Result};

pub struct Timers {
    pub fetch_and_redraw:
        Timer<{ IDT_FETCH_AND_REDRAW_TIMER.0 }, FETCH_TIMER_MS, FETCH_AND_REDRAW_TIMER_COALESCE>,
    pub tray_position:
        Timer<{ IDT_TRAY_POSITION_TIMER.0 }, TRAY_POSITION_TIMER_MS, TRAY_POSITION_TIMER_COALESCE>,
    pub z_order: Timer<{ IDT_Z_ORDER_TIMER.0 }, Z_ORDER_TIMER_MS, Z_ORDER_TIMER_COALESCE>,
    pub mic_state: Timer<{ IDT_MIC_STATE_TIMER.0 }, MIC_STATE_TIMER_MS, MIC_STATE_TIMER_COALESCE>,
}

impl Timers {
    pub fn new() -> Self {
        Self {
            fetch_and_redraw: Timer::new(),
            tray_position: Timer::new(),
            z_order: Timer::new(),
            mic_state: Timer::new(),
        }
    }
}

pub struct Timer<const ID: usize, const INTERVAL: u32, const COALESCE: u32> {
    _priv: (),
}

impl<const ID: usize, const INTERVAL: u32, const COALESCE: u32> Timer<ID, INTERVAL, COALESCE> {
    fn new() -> Self {
        Self { _priv: () }
    }

    /// Schedule the timer.
    ///
    /// If the timer is already running, this will overwrite it.
    pub fn reschedule(&self, window: HWND) {
        if let Err(e) = self.reschedule_fallible(window) {
            log::error!("Rescheduling timer with id {ID} failed: {e}");
        }
    }

    fn reschedule_fallible(&self, window: HWND) -> Result<()> {
        // Note: this timer will be destroyed when the window is destroyed.
        // (And in fact we can't destroy it manually, since the window handle will be invalid at that point.)
        match unsafe { SetCoalescableTimer(Some(window), ID, INTERVAL, None, COALESCE) } {
            0 => Err(Error::from_win32()),
            _ => Ok(()),
        }
    }

    /// Kill the timer.
    pub fn kill(&self, window: HWND) {
        if let Err(e) = self.kill_fallible(window) {
            log::error!("Killing timer with id {ID} failed: {e}");
        }
    }

    pub fn kill_fallible(&self, window: HWND) -> Result<()> {
        unsafe { KillTimer(Some(window), ID) }
    }
}
