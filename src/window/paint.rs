use crate::constants::{
    FIRST_LINE_MIDPOINT_OFFSET_FROM_TOP, LABEL_WIDTH, MICROPHONE_WARNING_WIDTH, RIGHT_COLUMN_WIDTH,
    SECOND_LINE_MIDPOINT_OFFSET_FROM_TOP,
};
use crate::defer;
use crate::metrics::Metrics;
use crate::utils::{RectExt, ScaleBy, ScalingFactor, Unscaled};
use std::cell::Cell;
use std::mem;
use std::ptr;
use windows::core::{w, Error, Result};
use windows::Win32::Foundation::{
    COLORREF, ERROR_DC_NOT_FOUND, ERROR_EMPTY, ERROR_FILE_NOT_FOUND, HWND, POINT, RECT,
};
use windows::Win32::Graphics::Gdi::{
    GetDC, GetMonitorInfoW, MonitorFromPoint, ReleaseDC, AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION,
    DT_NOCLIP, DT_NOPREFIX, DT_SINGLELINE, HDC, MONITORINFO, MONITOR_DEFAULTTOPRIMARY, RGBQUAD,
};
use windows::Win32::UI::Controls::{
    BeginBufferedPaint, BufferedPaintClear, BufferedPaintInit, BufferedPaintUnInit, CloseThemeData,
    DrawThemeTextEx, EndBufferedPaint, GetBufferedPaintBits, GetThemeTextExtent, BPBF_TOPDOWNDIB,
    BPPF_NOCLIP, BP_PAINTPARAMS, DTTOPTS, DTT_COMPOSITED, DTT_TEXTCOLOR, HTHEME, TEXT_BODYTEXT,
};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, OpenThemeDataForDpi};
use windows::Win32::UI::WindowsAndMessaging::{
    UpdateLayeredWindow, ULW_ALPHA, USER_DEFAULT_SCREEN_DPI,
};

pub struct Paint {
    /// SAFETY: must only be provided after calling `BufferedPaintInit`.
    called_buffered_paint_init: (),
    /// Whether to make the window more visible for debugging.
    pub debug: Cell<bool>,
    /// Offset from the right edge of the monitor, in unscaled pixels.
    pub offset_from_right: Cell<Unscaled<i32>>,
    /// Whether to always skip paint attempts.
    pub skip_paint: Cell<bool>,
    /// DPI scaling factor of the window.
    pub dpi: Cell<ScalingFactor>,
    /// Size and position of the window.
    pub rect: Cell<RECT>,
}

impl Drop for Paint {
    fn drop(&mut self) {
        _ = self.called_buffered_paint_init;
        // SAFETY: init and uninit must be called in pairs; we call init when constructing this type
        if let Err(e) = unsafe { BufferedPaintUnInit() } {
            log::error!("BufferedPaintUnInit failed: {}", e);
        }
    }
}

impl Paint {
    pub fn new(window: HWND) -> Result<Self> {
        let dpi = unsafe { GetDpiForWindow(window) };
        let dpi = ScalingFactor::from_ratio(dpi, USER_DEFAULT_SCREEN_DPI);

        // Window starts out not displayed, with size and position zero.
        let rect = RECT {
            top: 0,
            bottom: 0,
            left: 0,
            right: 0,
        };

        // Not using DEFAULT_OFFSET_FROM_RIGHT here so issues with sending config are obvious
        let offset_from_right = Unscaled::new(0);

        Ok(Self {
            debug: Cell::new(false),
            offset_from_right: Cell::new(offset_from_right),
            skip_paint: Cell::new(false),
            dpi: Cell::new(dpi),
            rect: Cell::new(rect),
            called_buffered_paint_init: {
                // SAFETY: init and uninit must be called in pairs; after this point, we construct self, so drop will call uninit
                unsafe { BufferedPaintInit()? }
            },
            // ...DO NOT add more fields after this...
        })
    }

    pub fn set_debug(&self, debug: bool) {
        self.debug.set(debug);
    }

    pub fn set_offset_from_right(&self, offset_from_right: Unscaled<i32>) {
        self.offset_from_right.set(offset_from_right);
    }

    pub fn set_skip_paint(&self, skip: bool) {
        self.skip_paint.set(skip);
    }

    pub fn set_dpi(&self, dpi: u32) -> ScalingFactor {
        let dpi = ScalingFactor::from_ratio(dpi, USER_DEFAULT_SCREEN_DPI);
        self.dpi.set(dpi);
        dpi
    }

    pub fn update_size_and_position(&self) {
        match self.compute_size_and_position() {
            Ok(rect) => {
                self.rect.set(rect);
            }
            Err(e) => {
                log::warn!(
                    "Update window position failed, preserving old position: {}",
                    e
                );
            }
        }
    }

    fn compute_size_and_position(&self) -> Result<RECT> {
        let dpi = self.dpi.get();

        // Get primary monitor (which always includes the origin)
        let monitor = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };

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

        let midpoint = |a, b| a + (b - a) / 2;

        // Height is always the size of the taskbar
        let top = monitor_info.rcWork.bottom;
        let bottom = monitor_info.rcMonitor.bottom;
        // Right edge the specified distance from the right edge of the screen
        let right = monitor_info.rcMonitor.right - self.offset_from_right.get().scale_by(dpi);
        // Left edge positioned at the horizontal center of the display, with enough room for the mic warning
        let left = midpoint(monitor_info.rcMonitor.left, monitor_info.rcMonitor.right)
            - MICROPHONE_WARNING_WIDTH.scale_by(dpi) / 2;

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

    /// Paint the window using the window's device context.
    pub fn render(&self, window: HWND, metrics: &Metrics, is_muted: bool) {
        if let Err(e) = self.render_fallible(window, metrics, is_muted) {
            log::error!("Paint failed: {}", e);
        }
    }

    /// Toplevel paint method, responsible for dealing with paint buffering and updating the window,
    /// but not with drawing any content.
    fn render_fallible(&self, window: HWND, metrics: &Metrics, is_muted: bool) -> Result<()> {
        if self.skip_paint.get() {
            return Ok(());
        }

        let rect = self.rect.get();
        let size = rect.size();
        let position = rect.top_left_corner();

        // Fetch win HDC so we can create temporary mem HDC of the same size.
        let win_hdc = unsafe { GetDC(Some(window)) };
        if win_hdc.is_invalid() {
            return Err(Error::from(ERROR_DC_NOT_FOUND));
        }
        defer! {
            _ = unsafe { ReleaseDC(Some(window), win_hdc) };
        }

        // Use buffered paint to draw into temporary mem HDC...
        let (hdc, buffered_paint) = {
            let mut hdc = HDC::default();
            let buffered_paint = unsafe {
                BeginBufferedPaint(
                    win_hdc,
                    &RECT::from_size(size),
                    // Required for us to manually write the background when debugging.
                    // Required for DTT_COMPOSITED to work.
                    // Always 8bpc, regardless of color depth of current monitor.
                    // (This isn't a big deal since we're only drawing white + transparency, so we don't need HDR.)
                    //
                    // Note that BPBF_COMPATIBLEBITMAP is recommended for hidpi applications:
                    // https://blogs.windows.com/windowsdeveloper/2017/05/19/improving-high-dpi-experience-gdi-based-desktop-apps/
                    // But as far as I can tell, this is only because it works with GDI scaling,
                    // which is a hack / compatibility layer to make non-DPI-aware apps render some elements at higher DPI.
                    // But we don't need GDI scaling, since we declare ourselves DPI aware, so none of our windows get scaled
                    // and we just draw everything normally (but with manually-computed larger sizes) at the physical screen resolution.
                    BPBF_TOPDOWNDIB,
                    Some(&BP_PAINTPARAMS {
                        cbSize: mem::size_of::<BP_PAINTPARAMS>() as u32,
                        dwFlags: BPPF_NOCLIP,
                        ..Default::default()
                    }),
                    &mut hdc,
                )
            };
            if buffered_paint == 0 {
                return Err(Error::from_win32());
            }
            (hdc, buffered_paint)
        };
        defer! {
            // ...and don't update (false) the underlying window...
            if let Err(e) = unsafe { EndBufferedPaint(buffered_paint, false) } {
                log::error!("EndBufferedPaint failed: {}", e);
            }
        }

        // Clear the temporary mem HDC, which may contain the previous contents of the window.
        unsafe { BufferedPaintClear(buffered_paint, None)? };

        // When debugging is enabled, fill in window background.
        // We have to do this manually because GDI brushes don't support alpha.
        if self.debug.get() {
            // Get the bits from the temporary mem HDC, along with the real width of each row (which may be larger than required).
            let mut bits = ptr::null_mut();
            let mut cx_row = 0;
            unsafe { GetBufferedPaintBits(buffered_paint, &mut bits, &mut cx_row)? };
            assert!(!bits.is_null());

            let cx_row: usize = cx_row.try_into().unwrap();
            let cx: usize = size.cx.try_into().unwrap();
            let cy: usize = size.cy.try_into().unwrap();
            assert!(cx_row >= cx);

            for y in 0..cy {
                for x in 0..cx {
                    // SAFETY: this is in bounds of the bitmap allocation per GetBufferedPaintBits contract
                    unsafe {
                        let elem = bits.add(y * cx_row + x);
                        *elem = RGBQUAD {
                            rgbRed: 0x77,
                            rgbGreen: 0x77,
                            rgbBlue: 0x00,
                            rgbReserved: 0xff, // alpha
                        };
                    }
                }
            }
        }

        // ...draw the content...
        self.draw_content(hdc, metrics, is_muted)?;

        // ...and then write the temporary mem HDC to the window, with alpha blending.
        unsafe {
            UpdateLayeredWindow(
                window,
                None,
                Some(&position),
                Some(&size),
                Some(hdc),
                Some(&POINT { x: 0, y: 0 }),
                COLORREF(0),
                Some(&BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8, // Use source alpha channel
                    ..Default::default()
                }),
                ULW_ALPHA,
            )?
        };

        Ok(())
    }

    /// Draw the window content to the given device context.
    fn draw_content(&self, hdc: HDC, metrics: &Metrics, is_muted: bool) -> Result<()> {
        let dpi = self.dpi.get();
        let size = self.rect.get().size();

        let text_style = unsafe {
            OpenThemeDataForDpi(None, w!("TEXTSTYLE"), USER_DEFAULT_SCREEN_DPI.scale_by(dpi))
        };
        if text_style.is_invalid() {
            return Err(Error::from(ERROR_FILE_NOT_FOUND));
        }
        defer! {
            if let Err(e) = unsafe { CloseThemeData(text_style) } {
                log::error!("CloseThemeData failed: {}", e);
            }
        }

        let middle_at = |x, y| {
            move |r: RECT| {
                r.with_horizontal_midpoint_at(x)
                    .with_vertical_midpoint_at(y)
            }
        };
        let right_mid_at =
            |x, y| move |r: RECT| r.with_right_edge_at(x).with_vertical_midpoint_at(y);
        let left_mid_at = |x, y| move |r: RECT| r.with_left_edge_at(x).with_vertical_midpoint_at(y);

        let text = |text: &str, position: &dyn Fn(RECT) -> RECT| {
            draw_text(hdc, text_style, text, position)
        };

        // Draw microphone warning if unmuted

        if !is_muted {
            text(
                ">>>>>>>>>>>>>>>> UNMUTED <<<<<<<<<<<<<<<<",
                &middle_at(MICROPHONE_WARNING_WIDTH.scale_by(dpi) / 2, size.cy / 2),
            )?;
        }

        // Draw metrics

        let cpu = metrics.avg_cpu_percent();
        let mem = metrics.avg_memory_percent();
        let net = metrics.avg_network_mbit();
        let dsk = metrics.avg_disk_mbyte();

        let right_column = size.cx - LABEL_WIDTH.scale_by(dpi);
        let left_column = size.cx - RIGHT_COLUMN_WIDTH.scale_by(dpi) - LABEL_WIDTH.scale_by(dpi);

        let first_line_midpoint = FIRST_LINE_MIDPOINT_OFFSET_FROM_TOP.scale_by(dpi);
        let second_line_midpoint = SECOND_LINE_MIDPOINT_OFFSET_FROM_TOP.scale_by(dpi);

        text(" CPU", &left_mid_at(right_column, first_line_midpoint))?;
        text(" RAM", &left_mid_at(right_column, second_line_midpoint))?;
        text(" NET", &left_mid_at(left_column, first_line_midpoint))?;
        text(" DSK", &left_mid_at(left_column, second_line_midpoint))?;

        text(
            &format!("{:.0}%", cpu),
            &right_mid_at(right_column, first_line_midpoint),
        )?;
        text(
            &format!("{:.0}%", mem),
            &right_mid_at(right_column, second_line_midpoint),
        )?;
        text(
            &format!("{:.0} Mb/s", net),
            &right_mid_at(left_column, first_line_midpoint),
        )?;
        text(
            &format!("{:.0} MB/s", dsk),
            &right_mid_at(left_column, second_line_midpoint),
        )?;

        Ok(())
    }
}

fn draw_text(
    hdc: HDC,
    text_style: HTHEME,
    text: &str,
    position: impl FnOnce(RECT) -> RECT,
) -> Result<()> {
    let text: &[u16] = &text.encode_utf16().collect::<Vec<_>>();

    let partid = TEXT_BODYTEXT;
    let stateid = 0;
    // > DrawText is somewhat faster when DT_NOCLIP is used.
    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-drawtext
    // (And we don't need clipping since we generate a rect that's the right size.)
    let textflags = DT_NOCLIP | DT_NOPREFIX | DT_SINGLELINE;

    // Get size of text
    let text_size =
        unsafe { GetThemeTextExtent(text_style, hdc, partid.0, stateid, text, textflags, None)? };

    // Move text into desired position
    let mut output_rect = position(text_size);

    unsafe {
        DrawThemeTextEx(
            text_style,
            hdc,
            partid.0,
            stateid,
            text,
            textflags,
            &mut output_rect,
            Some(&DTTOPTS {
                dwSize: mem::size_of::<DTTOPTS>() as u32,
                dwFlags: DTT_COMPOSITED | DTT_TEXTCOLOR,
                crText: COLORREF(0xffffff),
                ..Default::default()
            }),
        )?
    };

    Ok(())
}
