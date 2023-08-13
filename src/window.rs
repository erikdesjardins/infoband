use crate::defer;
use crate::ext::RectExt;
use crate::metrics;
use crate::module;
use crate::proc::{window_proc, ProcHandler};
use windows::core::{Error, Result, HRESULT, HSTRING};
use windows::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetTextExtentPointW, SetBkColor, TextOutW, DRAW_TEXT_FORMAT, HDC,
    PAINTSTRUCT,
};
use windows::Win32::UI::Controls::{
    BeginBufferedPaint, CloseThemeData, DrawThemeParentBackground, DrawThemeTextEx,
    EndBufferedPaint, OpenThemeData, BPBF_TOPDOWNDIB, DTTOPTS, DTT_COMPOSITED, DTT_GLOWSIZE,
    DTT_TEXTCOLOR,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DispatchMessageW, GetClientRect, GetMessageW, LoadCursorW, PostQuitMessage,
    RegisterClassW, ShowWindow, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, MSG, SW_SHOW,
    WINDOW_EX_STYLE, WM_DESTROY, WM_ERASEBKGND, WM_PAINT, WM_PRINTCLIENT, WNDCLASSW,
    WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_OVERLAPPEDWINDOW,
};

pub fn create_and_run_message_loop(bordered: bool) -> Result<()> {
    let instance = module::get_handle();

    // SAFETY: using predefined system cursor, so instance handle is unused; IDC_ARROW is guaranteed to exist
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW)? };

    let class = w!("infobandwindow");

    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        hCursor: cursor,
        hInstance: instance,
        lpszClassName: class,
        lpfnWndProc: Some(window_proc::<InfoBand>),
        ..Default::default()
    };

    // SAFETY: all necessary attributes of WNDCLASSW are initialized
    let atom = unsafe { RegisterClassW(&wc) };
    assert!(atom != 0);

    let mut style = WS_CLIPCHILDREN | WS_CLIPSIBLINGS;
    if bordered {
        style |= WS_OVERLAPPEDWINDOW;
    }

    // Note that this window will be destroyed by the default handler for WM_CLOSE
    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            None,
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            metrics::SIZE.x,
            metrics::SIZE.y,
            None,
            None,
            instance,
            None,
        )
    };

    // Show window after setting it up
    unsafe { ShowWindow(window, SW_SHOW) };

    run_message_loop()?;

    Ok(())
}

#[inline(never)]
pub fn run_message_loop() -> Result<()> {
    let mut msg = MSG::default();
    // SAFETY: msg pointer is valid
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        // SAFETY: msg pointer is valid
        unsafe { DispatchMessageW(&msg) };
    }

    // Apparently, wParam is the exit code
    let exit_code = msg.wParam.0 as i32;
    if exit_code == 0 {
        Ok(())
    } else {
        Err(Error::new(HRESULT(exit_code), HSTRING::new()))
    }
}

#[derive(Default)]
struct InfoBand {
    is_composition_enabled: bool,
}

impl InfoBand {
    fn paint_without_context(&self, window: HWND) {
        let mut ps = PAINTSTRUCT::default();
        // SAFETY: ps pointer is valid
        let hdc = unsafe { BeginPaint(window, &mut ps) };
        defer! {
            _ = unsafe { EndPaint(window, &ps) };
        }
        self.paint(window, hdc);
    }

    fn paint(&self, window: HWND, hdc: HDC) {
        if let Err(e) = self.paint_fallible(window, hdc) {
            log::error!("Paint failed: {}", e);
        }
    }

    fn paint_fallible(&self, window: HWND, hdc: HDC) -> Result<()> {
        let content_glass = w!("Test content glass");
        let content = w!("Test content");

        let mut rc = RECT::default();
        unsafe { GetClientRect(window, &mut rc) };

        let mut size = SIZE::default();

        if self.is_composition_enabled {
            let theme = unsafe { OpenThemeData(None, w!("BUTTON")) };
            if !theme.is_invalid() {
                defer! {
                    _ = unsafe { CloseThemeData(theme) };
                }

                let mut hdc_paint = HDC::default();
                let buffered_paint =
                    unsafe { BeginBufferedPaint(hdc, &rc, BPBF_TOPDOWNDIB, None, &mut hdc_paint) };
                defer! {
                    _ = unsafe { EndBufferedPaint(buffered_paint, true) };
                }

                unsafe { DrawThemeParentBackground(window, hdc_paint, Some(&rc))? };

                unsafe { GetTextExtentPointW(hdc, content_glass.as_wide(), &mut size).ok()? };
                let left = (rc.width() - size.cx) / 2;
                let top = (rc.height() - size.cy) / 2;
                let mut rc_text = RECT {
                    left,
                    top,
                    right: left + size.cx,
                    bottom: top + size.cy,
                };

                let dtt = DTTOPTS {
                    dwSize: std::mem::size_of::<DTTOPTS>() as u32,
                    dwFlags: DTT_COMPOSITED | DTT_TEXTCOLOR | DTT_GLOWSIZE,
                    crText: COLORREF(0xffff00),
                    iGlowSize: 10,
                    ..Default::default()
                };

                unsafe {
                    DrawThemeTextEx(
                        theme,
                        hdc_paint,
                        0,
                        0,
                        content_glass.as_wide(),
                        DRAW_TEXT_FORMAT(0),
                        &mut rc_text,
                        Some(&dtt),
                    )?
                };
            }
        } else {
            unsafe { SetBkColor(hdc, COLORREF(0xffff00)) };
            unsafe { GetTextExtentPointW(hdc, content.as_wide(), &mut size).ok()? };
            unsafe {
                TextOutW(
                    hdc,
                    (rc.width() - size.cx) / 2,
                    (rc.height() - size.cy) / 2,
                    content.as_wide(),
                )
                .ok()?
            };
        }

        Ok(())
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
        match message {
            WM_PAINT => {
                log::debug!("Starting repaint (WM_PAINT)");
                self.paint_without_context(window);
                None
            }
            WM_PRINTCLIENT => {
                log::debug!("Starting repaint (WM_PRINTCLIENT)");
                self.paint(window, HDC(wparam.0 as _));
                None
            }
            WM_ERASEBKGND => {
                log::debug!("Handling background erase (WM_ERASEBKGND)");
                let res = if self.is_composition_enabled { 1 } else { 0 };
                // Bypass the default window proc
                Some(LRESULT(res))
            }
            WM_DESTROY => {
                log::debug!("Shutting down (WM_DESTROY)");
                // SAFETY: no preconditions
                unsafe { PostQuitMessage(0) };
                None
            }
            _ => {
                log::trace!(
                    "Message handled by default window proc (message=0x{:04x}, wparam=0x{:016x} lparam=0x{:016x})",
                    message,
                    wparam.0,
                    lparam.0
                );
                None
            }
        }
    }
}
