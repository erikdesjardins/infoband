use crate::constants::WINDOW_SIZE;
use crate::defer;
use crate::util::RectExt;
use crate::window::state::InfoBand;
use std::ptr;
use windows::core::{Error, Result};
use windows::w;
use windows::Win32::Foundation::{COLORREF, ERROR_FILE_NOT_FOUND, HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetDC, ReleaseDC, AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION, DRAW_TEXT_FORMAT, HDC, RGBQUAD,
};
use windows::Win32::UI::Controls::{
    BeginBufferedPaint, CloseThemeData, DrawThemeTextEx, EndBufferedPaint, GetBufferedPaintBits,
    GetThemeTextExtent, OpenThemeData, BPBF_TOPDOWNDIB, BPPF_NOCLIP, BP_PAINTPARAMS, DTTOPTS,
    DTT_COMPOSITED, DTT_TEXTCOLOR, HTHEME,
};
use windows::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

impl InfoBand {
    /// Paint the window using the window's device context.
    pub fn paint(&self, window: HWND) {
        let win_hdc = unsafe { GetDC(window) };
        defer! {
            _ = unsafe { ReleaseDC(window, win_hdc) };
        }

        self.paint_to_context(window, win_hdc);
    }

    /// Paint the window to the given context.
    pub fn paint_to_context(&self, window: HWND, win_hdc: HDC) {
        if let Err(e) = self.paint_fallible(window, win_hdc) {
            log::error!("Paint failed: {}", e);
        }
    }

    /// Toplevel paint method, responsible for dealing with paint buffering and updating the window,
    /// but not with drawing any content.
    fn paint_fallible(&self, window: HWND, win_hdc: HDC) -> Result<()> {
        let rc = RECT::from_size(WINDOW_SIZE);

        // Use buffered paint to draw into temporary mem HDC...
        let mut hdc = HDC::default();
        let buffered_paint = unsafe {
            BeginBufferedPaint(
                win_hdc,
                &rc,
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

        // When debugging is enabled, fill in window background.
        // We have to do this manually because GDI brushes don't support alpha.
        if self.debug_paint.get() {
            // Get the bits from the temporary mem HDC, along with the real width of each row (which may be larger than required).
            let mut bits = ptr::null_mut();
            let mut cx_row = 0;
            unsafe { GetBufferedPaintBits(buffered_paint, &mut bits, &mut cx_row)? };
            assert!(!bits.is_null());

            let cx_row: usize = cx_row.try_into().unwrap();
            let cx: usize = rc.width().try_into().unwrap();
            let cy: usize = rc.height().try_into().unwrap();
            assert!(cx_row >= cx);

            for y in 0..cy {
                for x in 0..cx {
                    // SAFETY: this is in bounds of the bitmap allocation per GetBufferedPaintBits contract
                    unsafe {
                        let elem = bits.add(y * cx_row + x);
                        *elem = RGBQUAD {
                            rgbRed: 0x77,
                            rgbGreen: 0x00,
                            rgbBlue: 0x00,
                            rgbReserved: 0xff, // alpha
                        };
                    }
                }
            }
        }

        // ...draw the content...
        self.draw_content(hdc)?;

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

    /// Draw the window content to the given device context.
    fn draw_content(&self, hdc: HDC) -> Result<()> {
        let theme = unsafe { OpenThemeData(None, w!("BUTTON")) };
        if theme.is_invalid() {
            return Err(Error::from(ERROR_FILE_NOT_FOUND));
        }
        defer! {
            if let Err(e) = unsafe { CloseThemeData(theme) } {
                log::error!("CloseThemeData failed: {}", e);
            }
        }

        let top_right_corner_at = |x, y| move |r: RECT| r.with_right_edge_at(x).with_top_edge_at(y);

        draw_text(
            hdc,
            theme,
            unsafe { w!("Test content glass").as_wide() },
            top_right_corner_at(WINDOW_SIZE.cx, WINDOW_SIZE.cy / 2),
        )?;

        Ok(())
    }
}

fn draw_text(
    hdc: HDC,
    theme: HTHEME,
    text: &[u16],
    position: impl FnOnce(RECT) -> RECT,
) -> Result<()> {
    let partid = 0;
    let stateid = 0;
    let textflags = DRAW_TEXT_FORMAT(0);

    // Get size of text
    let text_size =
        unsafe { GetThemeTextExtent(theme, hdc, partid, stateid, text, textflags, None)? };

    // Move text into desired position
    let mut output_rect = position(text_size);

    unsafe {
        DrawThemeTextEx(
            theme,
            hdc,
            partid,
            stateid,
            text,
            textflags,
            &mut output_rect,
            Some(&DTTOPTS {
                dwSize: std::mem::size_of::<DTTOPTS>() as u32,
                dwFlags: DTT_COMPOSITED | DTT_TEXTCOLOR,
                crText: COLORREF(0xffff00),
                ..Default::default()
            }),
        )?
    };

    Ok(())
}
