use crate::module;
use crate::panic;
use std::ptr::NonNull;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use windows::core::Result;
use windows::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINTL, WPARAM};
use windows::Win32::Graphics::Gdi::InvalidateRect;
use windows::Win32::Graphics::Gdi::UpdateWindow;
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, ValidateRect};
use windows::Win32::UI::WindowsAndMessaging::SendMessageW;
use windows::Win32::UI::WindowsAndMessaging::WM_PRINTCLIENT;
use windows::Win32::UI::WindowsAndMessaging::WM_USER;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, LoadCursorW, RegisterClassW,
    SetWindowLongPtrW, ShowWindow, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, GWLP_USERDATA, IDC_ARROW,
    SW_HIDE, SW_SHOW, WINDOW_EX_STYLE, WM_ERASEBKGND, WM_NCCREATE, WM_NCDESTROY, WM_PAINT,
    WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
};

pub struct InfoBandWindow {
    handle: HWND,
}

impl Drop for InfoBandWindow {
    fn drop(&mut self) {
        // SAFETY: Window handle is valid as we created it
        unsafe { DestroyWindow(self.handle) };
    }
}

impl InfoBandWindow {
    /// Create a new InfoBandWindow as a child of the given window.
    /// The InfoBandWindow will not be visible until `show()` is called.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the given window is valid for the lifetime of the InfoBandWindow.
    pub unsafe fn new(parent: HWND) -> Result<Self> {
        let instance = module::get_handle();

        // SAFETY: using predefined system cursor, so instance handle is unused; IDC_ARROW is guaranteed to exist
        let cursor = unsafe { LoadCursorW(None, IDC_ARROW)? };

        let class = w!("infobandwindow");

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            hCursor: cursor,
            hInstance: instance,
            lpszClassName: class,
            lpfnWndProc: Some(window_proc),
            // TODO: for render testing
            hbrBackground: unsafe { CreateSolidBrush(COLORREF(0x00FF00)) },
            ..Default::default()
        };

        // SAFETY: all necessary attributes of WNDCLASSW are initialized
        let atom = unsafe { RegisterClassW(&wc) };
        assert!(atom != 0);

        // SAFETY: all non-option parameters are valid
        let handle = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                None,
                WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                parent,
                None,
                instance,
                None,
            )
        };

        Ok(Self { handle })
    }

    /// Get the window handle for this InfoBandWindow.
    ///
    /// Ownership of the window is retained by the InfoBandWindow, and will be destroyed on drop.
    pub fn handle(&self) -> HWND {
        self.handle
    }

    pub fn show(&self) {
        // SAFETY: Window handle is valid as we created it
        unsafe { ShowWindow(self.handle, SW_SHOW) };
    }

    pub fn hide(&self) {
        // SAFETY: Window handle is valid as we created it
        unsafe { ShowWindow(self.handle, SW_HIDE) };
    }

    pub fn compute_size(&self) -> POINTL {
        POINTL { x: 96, y: 24 }
    }

    pub fn invalidate(&self) {
        // SAFETY: Window handle is valid as we created it
        unsafe {
            // Triggers sending a WM_PAINT message to the window procedure
            InvalidateRect(self.handle, None, true);
        }
    }

    pub fn update(&self) {
        // SAFETY: Window handle is valid as we created it
        unsafe {
            // TODO: is this really necessary? it seems to send a WM_PAINT message like invalidate
            UpdateWindow(self.handle);
        }
    }

    pub fn set_composition_enabled(&self, enabled: bool) {
        if enabled {
            self.send_user_message(UM_SET_COMPOSITION_ENABLED);
        } else {
            self.send_user_message(UM_SET_COMPOSITION_DISABLED);
        }
    }

    fn send_user_message(&self, message: WPARAM) {
        // SAFETY: Window handle is valid as we created it
        unsafe { SendMessageW(self.handle, WM_USER, message, LPARAM(0)) };
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    panic::handle_unwind(|| {
        // Handle state/lifecycle management first
        let state = match message {
            WM_NCCREATE => {
                // SAFETY: handle is valid as we created it
                unsafe {
                    SetWindowLongPtrW(
                        window,
                        GWLP_USERDATA,
                        Box::into_raw(Box::new(InfoBandWindowProcState::default())) as isize,
                    )
                };
                // SAFETY: propagates same safety requirements as caller
                return unsafe { DefWindowProcW(window, message, wparam, lparam) };
            }
            WM_NCDESTROY => {
                // SAFETY: setting state to 0 is always safe; type will be InfoBandWindowProcState since we set it
                let state = unsafe {
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0) as *mut InfoBandWindowProcState
                };
                if !state.is_null() {
                    // SAFETY: state is either valid (as we set GWLP_USERDATA when constructing the window), or null
                    unsafe { drop(Box::from_raw(state)) };
                }
                // SAFETY: propagates same safety requirements as caller
                return unsafe { DefWindowProcW(window, message, wparam, lparam) };
            }
            _ => {
                // SAFETY: state is InfoBandWindowProcState, or null
                let state = unsafe {
                    GetWindowLongPtrW(window, GWLP_USERDATA) as *mut InfoBandWindowProcState
                };
                let state = NonNull::new(state);
                // SAFETY: state is either valid (as we set GWLP_USERDATA when constructing the window), or null
                unsafe { state.map(|s| s.as_ref()) }
            }
        };

        match (state, message) {
            (Some(state), WM_PAINT) => {
                state.paint(window, None);
            }
            (Some(state), WM_PRINTCLIENT) => {
                state.paint(window, Some(HDC(wparam.0 as _)));
            }
            (Some(state), WM_ERASEBKGND) => {
                let res = if state.composition_enabled() { 1 } else { 0 };
                // Bypass the default window proc
                return LRESULT(res);
            }
            (Some(state), WM_USER) => match wparam {
                UM_SET_COMPOSITION_DISABLED => {
                    state.set_composition_enabled(false);
                }
                UM_SET_COMPOSITION_ENABLED => {
                    state.set_composition_enabled(true);
                }
                _ => {
                    log::warn!(
                        "Unknown user message received (message={}, wparam={} lparam={})",
                        message,
                        wparam.0,
                        lparam.0
                    );
                }
            },
            (None, _) => {
                log::warn!(
                    "Window proc invoked after state destroyed (message={}, wparam={} lparam={})",
                    message,
                    wparam.0,
                    lparam.0
                );
            }
            (Some(_), _) => {
                log::trace!(
                    "Message handled by default window proc (message={}, wparam={} lparam={})",
                    message,
                    wparam.0,
                    lparam.0
                );
            }
        }

        // SAFETY: propagates same safety requirements as caller
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    })
    .unwrap_or(LRESULT(!0))
}

#[derive(Default)]
struct InfoBandWindowProcState {
    composition_enabled: AtomicBool,
}

const _ASSERT_SYNC: () = {
    // Window messages can be dispatched from multiple threads, so we need to be Sync
    const fn assert_sync<T: Sync>() {}
    assert_sync::<InfoBandWindowProcState>();
};

const UM_SET_COMPOSITION_DISABLED: WPARAM = WPARAM(0);
const UM_SET_COMPOSITION_ENABLED: WPARAM = WPARAM(1);

impl InfoBandWindowProcState {
    fn paint(&self, window: HWND, hdc: Option<HDC>) {
        // TODO: implement (OnPaint in https://learn.microsoft.com/en-us/windows/win32/shell/band-objects)
        // TODO: ValidateRect seems to not be necessary if we call BeginPaint
        unsafe { ValidateRect(window, None) };
    }

    fn composition_enabled(&self) -> bool {
        self.composition_enabled.load(Ordering::Relaxed)
    }

    fn set_composition_enabled(&self, enabled: bool) {
        self.composition_enabled.store(enabled, Ordering::Relaxed);
    }
}
