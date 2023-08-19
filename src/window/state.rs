use crate::constants::{IDT_REDRAW_TIMER, UM_ENABLE_DEBUG_PAINT, UM_INITIAL_PAINT};
use crate::proc::ProcHandler;
use std::cell::Cell;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::UI::Controls::{BufferedPaintInit, BufferedPaintUnInit};
use windows::Win32::UI::WindowsAndMessaging::{
    PostQuitMessage, WM_DESTROY, WM_ERASEBKGND, WM_PAINT, WM_PRINTCLIENT, WM_TIMER, WM_USER,
};

pub struct InfoBand {
    /// Whether or not `BufferedPaintInit` was called on construction.
    ///
    /// # Safety
    /// Must accurately reflect the state of whether `BufferedPaintInit` has been called.
    /// If this is true, it must be set to false iff `BufferedPaintUnInit` has been called.
    called_buffered_paint_init: bool,
    /// Whether to make the window more visible for debugging.
    pub debug_paint: Cell<bool>,
}

impl Default for InfoBand {
    fn default() -> Self {
        let called_buffered_paint_init = {
            let res = unsafe { BufferedPaintInit() };
            if let Err(e) = &res {
                log::error!("BufferedPaintInit failed: {}", e);
            }
            res.is_ok()
        };

        Self {
            called_buffered_paint_init,
            debug_paint: Cell::new(false),
        }
    }
}

impl Drop for InfoBand {
    fn drop(&mut self) {
        if self.called_buffered_paint_init {
            if let Err(e) = unsafe { BufferedPaintUnInit() } {
                log::error!("BufferedPaintUnInit failed: {}", e);
            }
        }
    }
}

impl ProcHandler for InfoBand {
    fn handle(
        &self,
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        Some(match message {
            WM_PAINT => {
                log::debug!("Starting repaint (WM_PAINT)");
                self.paint(window);
                LRESULT(0)
            }
            WM_PRINTCLIENT => {
                log::debug!("Starting repaint (WM_PRINTCLIENT)");
                self.paint_to_context(window, HDC(wparam.0 as _));
                LRESULT(0)
            }
            WM_ERASEBKGND => {
                log::debug!("Ignoring background erase (WM_ERASEBKGND)");
                // Since we use compositing, we don't need to erase the background
                LRESULT(1)
            }
            WM_DESTROY => {
                log::debug!("Shutting down (WM_DESTROY)");
                // SAFETY: no preconditions
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            WM_USER => match wparam {
                UM_ENABLE_DEBUG_PAINT => {
                    log::debug!("Enabling debug paint (UM_ENABLE_DEBUG_PAINT)");
                    self.debug_paint.set(true);
                    LRESULT(0)
                }
                UM_INITIAL_PAINT => {
                    log::debug!("Starting repaint (UM_INITIAL_PAINT)");
                    self.paint(window);
                    LRESULT(0)
                }
                _ => {
                    log::warn!("Unhandled user message (message=WM_USER, wparam/user_message=0x{:016x} lparam=0x{:016x})", wparam.0, lparam.0);
                    return None;
                }
            },
            WM_TIMER => match wparam {
                IDT_REDRAW_TIMER => {
                    log::debug!("Starting repaint (IDT_REDRAW_TIMER)");
                    self.paint(window);
                    LRESULT(0)
                }
                _ => {
                    log::warn!("Unhandled timer message (message=WM_TIMER, wparam/timer_id=0x{:016x} lparam=0x{:016x})", wparam.0, lparam.0);
                    return None;
                }
            },
            _ => {
                log::trace!(
                    "Default window proc (message=0x{:04x}, wparam=0x{:016x} lparam=0x{:016x})",
                    message,
                    wparam.0,
                    lparam.0
                );
                return None;
            }
        })
    }
}
