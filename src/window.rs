use crate::constants::{
    IDT_REDRAW_TIMER, REDRAW_TIMER_MS, UM_ENABLE_DEBUG_PAINT, UM_INITIAL_PAINT, WINDOW_SIZE,
};
use crate::defer;
use crate::module;
use crate::proc::window_proc;
use windows::core::{Error, Result, HRESULT, HSTRING};
use windows::w;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DispatchMessageW, GetMessageW, KillTimer, LoadCursorW, PostMessageW,
    RegisterClassW, SetTimer, ShowWindow, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, MSG,
    SW_SHOW, WM_USER, WNDCLASSW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_LAYERED,
    WS_EX_TRANSPARENT, WS_POPUP,
};

mod paint;
mod state;

pub fn create_and_run_message_loop(debug_paint: bool) -> Result<()> {
    let instance = module::get_handle();

    // SAFETY: using predefined system cursor, so instance handle is unused; IDC_ARROW is guaranteed to exist
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW)? };

    let class = w!("infobandwindow");

    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        hCursor: cursor,
        hInstance: instance,
        lpszClassName: class,
        lpfnWndProc: Some(window_proc::<state::InfoBand>),
        ..Default::default()
    };

    // SAFETY: all necessary attributes of WNDCLASSW are initialized
    let atom = unsafe { RegisterClassW(&wc) };
    assert!(atom != 0);

    // Note that this window will be destroyed by the default handler for WM_CLOSE
    let window = {
        // Layered window allows transparency
        // https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#layered-windows
        let mut exstyle = WS_EX_LAYERED;
        if !debug_paint {
            // Transparent window allows clicks to pass through everywhere
            // (Layered windows allow clicks to pass through in transparent areas only)
            exstyle |= WS_EX_TRANSPARENT;
        }

        let style = WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS;

        let window = unsafe {
            CreateWindowExW(
                exstyle,
                class,
                None,
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                WINDOW_SIZE.cx,
                WINDOW_SIZE.cy,
                None,
                None,
                instance,
                None,
            )
        };
        if window.0 == 0 {
            return Err(Error::from_win32());
        }
        window
    };

    // Enqueue a message to tell the window about debug settings
    if debug_paint {
        unsafe { PostMessageW(window, WM_USER, UM_ENABLE_DEBUG_PAINT, LPARAM(0)).ok()? };
    }

    // Enqueue a message for initial paint
    unsafe { PostMessageW(window, WM_USER, UM_INITIAL_PAINT, LPARAM(0)).ok()? };

    // Set up timer to redraw window periodically
    if unsafe { SetTimer(window, IDT_REDRAW_TIMER.0, REDRAW_TIMER_MS, None) } == 0 {
        return Err(Error::from_win32());
    }
    defer! {
        if let Err(e) = unsafe { KillTimer(window, IDT_REDRAW_TIMER.0).ok() } {
            log::error!("KillTimer failed: {}", e);
        }
    };

    // Show window after setting it up
    // (Note that layered windows still don't render until you call UpdateLayeredWindow)
    unsafe { ShowWindow(window, SW_SHOW) };

    // Run message loop (will block)
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
