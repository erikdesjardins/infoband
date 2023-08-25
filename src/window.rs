use crate::constants::{
    FETCH_TIMER_COALESCE, FETCH_TIMER_MS, IDT_FETCH_TIMER, IDT_REDRAW_TIMER, REDRAW_TIMER_COALESCE,
    REDRAW_TIMER_MS, UM_ENABLE_DEBUG_PAINT, UM_INITIAL_METRICS, UM_INITIAL_PAINT,
    UM_INITIAL_Z_ORDER,
};
use crate::window::proc::window_proc;
use windows::core::{w, Error, Result, HRESULT, HSTRING};
use windows::Win32::Foundation::{HINSTANCE, LPARAM};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DispatchMessageW, GetMessageW, LoadCursorW, PostMessageW, RegisterClassW,
    RegisterShellHookWindow, SetCoalescableTimer, ShowWindow, CS_HREDRAW, CS_VREDRAW,
    CW_USEDEFAULT, IDC_ARROW, MSG, SW_SHOWNA, WM_USER, WNDCLASSW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};

mod messages;
mod paint;
mod proc;
mod state;
mod z_order;

/// Make this process HiDPI aware.
/// Must be called before any other windowing functions.
pub fn make_process_dpi_aware() -> Result<()> {
    // SAFETY: not unsafe...
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)? };

    Ok(())
}

/// Create the toplevel window, start timers for updating it, and pump the windows message loop.
pub fn create_and_run_message_loop(instance: HINSTANCE, debug_paint: bool) -> Result<()> {
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

    // Note: this window will be destroyed by the default handler for WM_CLOSE.
    let window = {
        let window = unsafe {
            CreateWindowExW(
                // Layered window allows transparency:
                // https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#layered-windows
                // Transparent window allows clicks to pass through everywhere.
                // (Layered windows allow clicks to pass through in transparent areas only.)
                // Tool window hides it from the taskbar.
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                class,
                None,
                // Popup window removes borders and title bar.
                // Clipping probably not necessary since we don't have child windows.
                WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                // Leave all positions defaulted.
                // Layered windows aren't displayed until UpdateLayeredWindow is called, so we'll set it then.
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
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

    // Register window to receive shell hook messages. We use these to follow the z-order state of the taskbar.
    unsafe { RegisterShellHookWindow(window).ok()? };

    // Enqueue a message to tell the window about debug settings
    if debug_paint {
        unsafe { PostMessageW(window, WM_USER, UM_ENABLE_DEBUG_PAINT, LPARAM(0))? };
    }

    // Enqueue a message for initial metrics fetch
    unsafe { PostMessageW(window, WM_USER, UM_INITIAL_METRICS, LPARAM(0))? };

    // Enqueue a message for initial z-order update
    unsafe { PostMessageW(window, WM_USER, UM_INITIAL_Z_ORDER, LPARAM(0))? };

    // Enqueue a message for initial paint
    unsafe { PostMessageW(window, WM_USER, UM_INITIAL_PAINT, LPARAM(0))? };

    // Set up timer to fetch metrics.
    // Note: this timer will be destroyed when the window is destroyed. (And in fact we can't destroy it manually, since the window handle will be invalid.)
    if unsafe {
        SetCoalescableTimer(
            window,
            IDT_FETCH_TIMER.0,
            FETCH_TIMER_MS,
            None,
            FETCH_TIMER_COALESCE,
        )
    } == 0
    {
        return Err(Error::from_win32());
    }

    // Set up timer to redraw window periodically.
    // Note: this timer will be destroyed when the window is destroyed. (And in fact we can't destroy it manually, since the window handle will be invalid.)
    if unsafe {
        SetCoalescableTimer(
            window,
            IDT_REDRAW_TIMER.0,
            REDRAW_TIMER_MS,
            None,
            REDRAW_TIMER_COALESCE,
        )
    } == 0
    {
        return Err(Error::from_win32());
    }

    // Show window (without activating/focusing it) after setting it up.
    // Note that layered windows still don't render until you call UpdateLayeredWindow.
    unsafe { ShowWindow(window, SW_SHOWNA) };

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
