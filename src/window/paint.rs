use crate::constants::WINDOW_SIZE;
use crate::defer;
use crate::util::RectExt;
use crate::window::state::InfoBand;
use windows::core::{Error, Result};
use windows::w;
use windows::Win32::Foundation::{COLORREF, ERROR_FILE_NOT_FOUND, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    FillRect, GetDC, GetTextExtentPointW, ReleaseDC, AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION,
    COLOR_INFOBK, DRAW_TEXT_FORMAT, HBRUSH, HDC,
};
use windows::Win32::UI::Controls::{
    BeginBufferedPaint, CloseThemeData, DrawThemeTextEx, EndBufferedPaint, OpenThemeData,
    BPBF_TOPDOWNDIB, BPPF_NOCLIP, BP_PAINTPARAMS, DTTOPTS, DTT_COMPOSITED, DTT_TEXTCOLOR,
};
use windows::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

impl InfoBand {
    pub fn paint_without_context(&self, window: HWND) {
        let win_hdc = unsafe { GetDC(window) };
        defer! {
            _ = unsafe { ReleaseDC(window, win_hdc) };
        }

        self.paint(window, win_hdc);
    }

    pub fn paint(&self, window: HWND, win_hdc: HDC) {
        if let Err(e) = self.paint_fallible(window, win_hdc) {
            log::error!("Paint failed: {}", e);
        }
    }

    fn paint_fallible(&self, window: HWND, win_hdc: HDC) -> Result<()> {
        let content_glass = w!("Test content glass");

        let theme = unsafe { OpenThemeData(None, w!("BUTTON")) };
        if theme.is_invalid() {
            return Err(Error::from(ERROR_FILE_NOT_FOUND));
        }
        defer! {
            if let Err(e) = unsafe { CloseThemeData(theme) } {
                log::error!("CloseThemeData failed: {}", e);
            }
        }

        // Use buffered paint to draw into temporary mem HDC...
        let mut hdc = HDC::default();
        let buffered_paint = unsafe {
            BeginBufferedPaint(
                win_hdc,
                &RECT::from_size(WINDOW_SIZE),
                BPBF_TOPDOWNDIB,
                Some(&BP_PAINTPARAMS {
                    cbSize: std::mem::size_of::<BP_PAINTPARAMS>() as u32,
                    dwFlags: BPPF_NOCLIP,
                    ..Default::default()
                }),
                &mut hdc,
            )
        };
        defer! {
            // ...and don't update (false) the underlying window...
            if let Err(e) = unsafe { EndBufferedPaint(buffered_paint, false) } {
                log::error!("EndBufferedPaint failed: {}", e);
            }
        }

        if self.debug_paint.get() {
            unsafe {
                FillRect(
                    hdc,
                    &RECT::from_size(WINDOW_SIZE),
                    HBRUSH((COLOR_INFOBK.0 + 1) as isize),
                )
            };
        }

        let mut text_size = SIZE::default();
        unsafe { GetTextExtentPointW(hdc, content_glass.as_wide(), &mut text_size).ok()? };
        let left = (WINDOW_SIZE.cx - text_size.cx) / 2;
        let top = (WINDOW_SIZE.cy - text_size.cy) / 2;
        let mut rc_text = RECT {
            left,
            top,
            right: left + text_size.cx,
            bottom: top + text_size.cy,
        };

        unsafe {
            DrawThemeTextEx(
                theme,
                hdc,
                0,
                0,
                content_glass.as_wide(),
                DRAW_TEXT_FORMAT(0),
                &mut rc_text,
                Some(&DTTOPTS {
                    dwSize: std::mem::size_of::<DTTOPTS>() as u32,
                    dwFlags: DTT_COMPOSITED | DTT_TEXTCOLOR,
                    crText: COLORREF(0xffff00),
                    ..Default::default()
                }),
            )?
        };

        // ...and then write the temporary mem HDC to the window, with alpha blending.
        unsafe {
            UpdateLayeredWindow(
                window,
                None,
                None,
                Some(&WINDOW_SIZE),
                hdc,
                Some(&POINT { x: 0, y: 0 }),
                None,
                Some(&BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8, // Use source alpha channel
                    ..Default::default()
                }),
                ULW_ALPHA,
            )
            .ok()?
        };

        Ok(())
    }
}
