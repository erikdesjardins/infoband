use crate::constants::{
    HOTKEY_MIC_MUTE, HSHELL_RUDEAPPACTIVATED, HSHELL_WINDOWACTIVATED, IDT_FETCH_TIMER,
    IDT_MIC_STATE_TIMER, IDT_REDRAW_TIMER, IDT_Z_ORDER_TIMER, UM_ENABLE_DEBUG_PAINT,
    UM_INITIAL_METRICS, UM_INITIAL_MIC_STATE, UM_INITIAL_PAINT, UM_INITIAL_Z_ORDER,
    UM_SET_OFFSET_FROM_RIGHT, WTS_SESSION_LOCK, WTS_SESSION_LOGOFF, WTS_SESSION_LOGON,
    WTS_SESSION_UNLOCK,
};
use crate::metrics::Metrics;
use crate::utils::{ScaleBy, Unscaled};
use crate::window::messages;
use crate::window::microphone::Microphone;
use crate::window::paint::Paint;
use crate::window::proc::ProcHandler;
use crate::window::z_order::ZOrder;
use windows::core::{w, Error, Result};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    PostQuitMessage, RegisterWindowMessageW, WM_DESTROY, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_ERASEBKGND, WM_HOTKEY, WM_NCCALCSIZE, WM_NCPAINT, WM_PAINT, WM_TIMER, WM_USER,
    WM_WTSSESSION_CHANGE,
};

pub struct InfoBand {
    /// The message ID of the SHELLHOOK message.
    shellhook_message: u32,
    /// Paint state.
    paint: Paint,
    /// Z=order state.
    z_order: ZOrder,
    /// Microphone state.
    mic: Microphone,
    /// Performance metrics.
    metrics: Metrics,
}

impl ProcHandler for InfoBand {
    fn new(window: HWND) -> Result<Self> {
        let shellhook_message = {
            let res = unsafe { RegisterWindowMessageW(w!("SHELLHOOK")) };
            if res == 0 {
                return Err(Error::from_win32());
            }
            res
        };

        Ok(Self {
            shellhook_message,
            paint: Paint::new(window)?,
            z_order: ZOrder::new()?,
            mic: Microphone::new(window)?,
            metrics: Metrics::new()?,
        })
    }

    fn handle(
        &self,
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        Some(match message {
            WM_NCCALCSIZE => {
                log::debug!("Computing size of client area (WM_NCCALCSIZE)");
                // Handling this is required to ensure our window has no frame (even though it wouldn't be visible).
                // https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-nccalcsize
                if wparam.0 != 0 {
                    // > When wParam is TRUE, simply returning 0 without processing the NCCALCSIZE_PARAMS rectangles
                    // > will cause the client area to resize to the size of the window, including the window frame.
                    // > This will remove the window frame and caption items from your window, leaving only the client area displayed.
                    //
                    // This is exactly what we want.
                    LRESULT(0)
                } else {
                    // > If wParam is FALSE, lParam points to a RECT structure.
                    // > On entry, the structure contains the proposed window rectangle for the window.
                    // > On exit, the structure should contain the screen coordinates of the corresponding window client area.
                    //
                    // Similarly, we want the client area to take up the entire window size, so do nothing here as well.
                    //
                    // > If the wParam parameter is FALSE, the application should return zero.
                    LRESULT(0)
                }
            }
            WM_NCPAINT => {
                log::debug!("Ignoring frame repaint (WM_NCPAINT)");
                // We don't have a frame, so don't paint it.
                LRESULT(0)
            }
            WM_PAINT => {
                log::debug!("Ignoring client repaint (WM_PAINT)");
                // Layered windows don't have to handle WM_PAINT.
                // We do need to revalidate the window (here: let DefWindowProc do so),
                // or Windows will send us an endless stream of paint requests.
                return None;
            }
            WM_ERASEBKGND => {
                log::debug!("Ignoring background erase (WM_ERASEBKGND)");
                // Since we use compositing, we don't need to erase the background.
                LRESULT(1)
            }
            WM_DPICHANGED => {
                // Low 16 bits contains DPI
                let dpi_raw = u32::from(wparam.0 as u16);
                let dpi = self.paint.set_dpi(dpi_raw);
                log::info!(
                    "DPI changed to {} or {}% (WM_DPICHANGED)",
                    dpi_raw,
                    100.scale_by(dpi)
                );
                self.paint.update_size_and_position();
                self.paint
                    .render(window, &self.metrics, self.mic.is_muted());
                LRESULT(0)
            }
            WM_DISPLAYCHANGE => {
                log::debug!("Display resolution changed (WM_DISPLAYCHANGE)");
                self.paint.update_size_and_position();
                self.paint
                    .render(window, &self.metrics, self.mic.is_muted());
                LRESULT(0)
            }
            WM_DESTROY => {
                log::info!("Shutting down (WM_DESTROY)");
                // SAFETY: no preconditions
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            WM_USER => match wparam {
                UM_ENABLE_DEBUG_PAINT => {
                    log::info!("Enabling debug paint (UM_ENABLE_DEBUG_PAINT)");
                    self.paint.set_debug(true);
                    LRESULT(0)
                }
                UM_SET_OFFSET_FROM_RIGHT => {
                    let offset_from_right = Unscaled::new(lparam.0 as _);
                    log::info!(
                        "Setting offset from right to {} (UM_SET_OFFSET_FROM_RIGHT)",
                        offset_from_right
                    );
                    self.paint.set_offset_from_right(offset_from_right);
                    LRESULT(0)
                }
                UM_INITIAL_MIC_STATE => {
                    log::info!("Initial mic state update (UM_INITIAL_MIC_STATE)");
                    self.mic.refresh_devices();
                    self.mic.update_muted_state();
                    LRESULT(0)
                }
                UM_INITIAL_METRICS => {
                    log::info!("Initial metrics fetch (UM_INITIAL_METRICS)");
                    self.metrics.fetch();
                    LRESULT(0)
                }
                UM_INITIAL_Z_ORDER => {
                    log::info!("Initial z-order update (UM_INITIAL_Z_ORDER)");
                    self.z_order.touch_window(window);
                    self.z_order.update(window);
                    LRESULT(0)
                }
                UM_INITIAL_PAINT => {
                    log::info!("Initial paint (UM_INITIAL_PAINT)");
                    self.paint.update_size_and_position();
                    self.paint
                        .render(window, &self.metrics, self.mic.is_muted());
                    LRESULT(0)
                }
                _ => {
                    log::warn!(
                        "Unhandled user message (WM_USER id=0x{:08x} lparam=0x{:012x})",
                        wparam.0,
                        lparam.0
                    );
                    return None;
                }
            },
            _ if message == self.shellhook_message => match (wparam, lparam) {
                (HSHELL_RUDEAPPACTIVATED, LPARAM(0)) => {
                    // This seems to indicate that the taskbar itself was focused,
                    // so we need to re-set ourselves to TOPMOST to stay on top.
                    log::debug!(
                        "Reapplying z-order due to shell focus (SHELLHOOK id=0x{:08x})",
                        wparam.0
                    );
                    self.z_order.update(window);
                    LRESULT(0)
                }
                (
                    HSHELL_WINDOWACTIVATED | HSHELL_RUDEAPPACTIVATED | WPARAM(0x35) | WPARAM(0x36),
                    _,
                ) => {
                    // Per https://github.com/dechamps/RudeWindowFixer#the-rude-window-manager,
                    // these are the messages that the shell uses to update its z-order.
                    log::debug!(
                        "Queuing z-order check (SHELLHOOK id=0x{:08x} lparam=0x{:012x})",
                        wparam.0,
                        lparam.0
                    );
                    self.z_order.queue_update(window);
                    LRESULT(0)
                }
                _ => {
                    log::debug!(
                        "Ignoring shellhook message (SHELLHOOK id=0x{:08x} lparam=0x{:012x})",
                        wparam.0,
                        lparam.0
                    );
                    LRESULT(0)
                }
            },
            WM_WTSSESSION_CHANGE => match wparam {
                WTS_SESSION_LOGON => {
                    log::debug!("Resuming paint due to logon (WTS_SESSION_LOGON)");
                    self.paint.set_skip_paint(false);
                    LRESULT(0)
                }
                WTS_SESSION_LOGOFF => {
                    log::debug!("Skipping paint due to logoff (WTS_SESSION_LOGOFF)");
                    self.paint.set_skip_paint(true);
                    LRESULT(0)
                }
                WTS_SESSION_LOCK => {
                    log::debug!("Skipping paint due to lock (WTS_SESSION_LOCK)");
                    self.paint.set_skip_paint(true);
                    LRESULT(0)
                }
                WTS_SESSION_UNLOCK => {
                    log::debug!("Resuming paint due to unlock (WTS_SESSION_UNLOCK)");
                    self.paint.set_skip_paint(false);
                    LRESULT(0)
                }
                _ => {
                    log::debug!(
                        "Ignoring session change message (WM_WTSSESSION_CHANGE id=0x{:08x} lparam=0x{:012x})",
                        wparam.0,
                        lparam.0
                    );
                    LRESULT(0)
                }
            },
            WM_HOTKEY => match wparam {
                HOTKEY_MIC_MUTE => {
                    let mute = !self.mic.is_muted();
                    log::debug!("Toggling mic mute (WM_HOTKEY mute={})", mute);
                    self.mic.refresh_devices();
                    self.mic.set_mute(mute);
                    LRESULT(0)
                }
                _ => {
                    log::debug!(
                        "Ignoring hotkey message (WM_HOTKEY id=0x{:08x} lparam=0x{:012x})",
                        wparam.0,
                        lparam.0
                    );
                    LRESULT(0)
                }
            },
            WM_TIMER => match wparam {
                IDT_FETCH_TIMER => {
                    log::trace!("Fetching metrics (IDT_FETCH_TIMER)");
                    self.metrics.fetch();
                    LRESULT(0)
                }
                IDT_REDRAW_TIMER => {
                    log::trace!("Starting repaint (IDT_REDRAW_TIMER)");
                    self.paint
                        .render(window, &self.metrics, self.mic.is_muted());
                    LRESULT(0)
                }
                IDT_MIC_STATE_TIMER => {
                    self.mic.kill_timer(window);
                    let was_muted = self.mic.is_muted();
                    self.mic.update_muted_state();
                    let now_muted = self.mic.is_muted();
                    log::debug!(
                        "Checking mic state (IDT_MIC_STATE_TIMER was_muted={} now_muted={})",
                        was_muted,
                        now_muted
                    );
                    if was_muted != now_muted {
                        self.paint.render(window, &self.metrics, now_muted);
                    }
                    LRESULT(0)
                }
                IDT_Z_ORDER_TIMER => {
                    log::debug!("Rechecking z-order (IDT_Z_ORDER_TIMER)",);
                    self.z_order.kill_timer(window);
                    self.z_order.update(window);
                    LRESULT(0)
                }
                _ => {
                    log::warn!(
                        "Unhandled timer message (WM_TIMER id=0x{:08x} lparam=0x{:012x})",
                        wparam.0,
                        lparam.0
                    );
                    return None;
                }
            },
            _ => {
                log::debug!(
                    "Default window proc ({} wparam=0x{:08x} lparam=0x{:012x})",
                    messages::Name(message),
                    wparam.0,
                    lparam.0
                );
                return None;
            }
        })
    }
}
