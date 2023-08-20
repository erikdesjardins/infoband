use crate::constants::{IDT_REDRAW_TIMER, UM_ENABLE_DEBUG_PAINT, UM_INITIAL_PAINT};
use crate::utils::{ScaleBy, ScalingFactor};
use crate::window::messages;
use crate::window::proc::ProcHandler;
use std::cell::Cell;
use windows::core::Result;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::UI::Controls::{BufferedPaintInit, BufferedPaintUnInit};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    PostQuitMessage, USER_DEFAULT_SCREEN_DPI, WM_DESTROY, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_ERASEBKGND, WM_PAINT, WM_PRINTCLIENT, WM_TIMER, WM_USER,
};

pub struct InfoBand {
    /// SAFETY: must only be provided after calling `BufferedPaintInit`.
    called_buffered_paint_init: (),
    /// Whether to make the window more visible for debugging.
    pub debug_paint: Cell<bool>,
    /// Current DPI scaling factor of the window.
    pub dpi: Cell<ScalingFactor>,
    /// Current size of the window.
    pub size: Cell<SIZE>,
    /// Current position of the window.
    pub position: Cell<POINT>,
}

impl Drop for InfoBand {
    fn drop(&mut self) {
        _ = self.called_buffered_paint_init;
        // SAFETY: init and uninit must be called in pairs; we call init when constructing this type
        if let Err(e) = unsafe { BufferedPaintUnInit() } {
            log::error!("BufferedPaintUnInit failed: {}", e);
        }
    }
}

impl ProcHandler for InfoBand {
    fn create(window: HWND) -> Result<Self> {
        let dpi = unsafe { GetDpiForWindow(window) };
        let dpi = ScalingFactor::from_ratio(dpi, USER_DEFAULT_SCREEN_DPI);

        // Window starts out not displayed, with size and position zero.
        let size = SIZE { cx: 0, cy: 0 };
        let position = POINT { x: 0, y: 0 };

        Ok(Self {
            debug_paint: Cell::new(false),
            dpi: Cell::new(dpi),
            size: Cell::new(size),
            position: Cell::new(position),
            called_buffered_paint_init: {
                // SAFETY: init and uninit must be called in pairs; after this point, we construct self, so drop will call uninit
                unsafe { BufferedPaintInit()? }
            },
            // ...DO NOT add more fields after this...
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
            WM_DPICHANGED => {
                // Low 16 bits contains DPI
                let dpi_raw = u32::from(wparam.0 as u16);
                let dpi = ScalingFactor::from_ratio(dpi_raw, USER_DEFAULT_SCREEN_DPI);
                log::debug!(
                    "DPI changed to {} or {}% (WM_DPICHANGED)",
                    dpi_raw,
                    100.scale_by(dpi)
                );
                self.dpi.set(dpi);
                self.compute_size_and_position();
                self.paint(window);
                LRESULT(0)
            }
            WM_DISPLAYCHANGE => {
                log::debug!("Display resolution changed (WM_DISPLAYCHANGE)");
                self.compute_size_and_position();
                self.paint(window);
                LRESULT(0)
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
                    log::debug!("Starting paint (UM_INITIAL_PAINT)");
                    self.compute_size_and_position();
                    self.paint(window);
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
            WM_TIMER => match wparam {
                IDT_REDRAW_TIMER => {
                    log::debug!("Starting repaint (IDT_REDRAW_TIMER)");
                    self.paint(window);
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
                log::trace!(
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
