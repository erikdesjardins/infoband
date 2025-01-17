use crate::constants::{
    FETCH_TIMER_COALESCE, FETCH_TIMER_MS, IDT_FETCH_TIMER, IDT_REDRAW_TIMER, REDRAW_TIMER_COALESCE,
    REDRAW_TIMER_MS, UM_ENABLE_DEBUG_PAINT, UM_INITIAL_METRICS, UM_INITIAL_PAINT,
    UM_INITIAL_Z_ORDER, UM_SET_OFFSET_FROM_RIGHT,
};
use crate::utils::Unscaled;
use crate::window::proc::window_proc;
use windows::core::{w, Error, Result, HRESULT};
use windows::Win32::Foundation::{HINSTANCE, LPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
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

/// Create the toplevel window, start timers for updating it, and pump the windows message loop.
pub fn create_and_run_message_loop(
    offset_from_right: Unscaled<i32>,
    debug_paint: bool,
) -> Result<()> {
    // SAFETY: no safety requirements when passing null
    let instance = HINSTANCE::from(unsafe { GetModuleHandleW(None)? });

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
            Some(instance),
            None,
        )?
    };

    // Register window to receive shell hook messages. We use these to follow the z-order state of the taskbar.
    unsafe { RegisterShellHookWindow(window).ok()? };

    // Register window to receive session notifictions. We use these to stop drawing when the session is locked.
    // The main reason we do this is to avoid weird situations where buffered paint gets stuck in a failed state.
    // This seems to happen when we attempt to draw when the monitor that our window is on is turned off, e.g. if when waking from sleep,
    // a different monitor in a multi-monitor setup wakes up first.
    unsafe { WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION)? };

    // Enqueue a message to tell the window about debug settings
    if debug_paint {
        unsafe { PostMessageW(Some(window), WM_USER, UM_ENABLE_DEBUG_PAINT, LPARAM(0))? };
    }

    // Enqueue a message to tell the window about the offset from the right edge of the screen
    let offset = offset_from_right.into_inner() as _;
    unsafe {
        PostMessageW(
            Some(window),
            WM_USER,
            UM_SET_OFFSET_FROM_RIGHT,
            LPARAM(offset),
        )?
    };

    // Enqueue a message for initial metrics fetch
    unsafe { PostMessageW(Some(window), WM_USER, UM_INITIAL_METRICS, LPARAM(0))? };

    // Enqueue a message for initial z-order update
    unsafe { PostMessageW(Some(window), WM_USER, UM_INITIAL_Z_ORDER, LPARAM(0))? };

    // Enqueue a message for initial paint
    unsafe { PostMessageW(Some(window), WM_USER, UM_INITIAL_PAINT, LPARAM(0))? };

    // Set up timer to fetch metrics.
    // Note: this timer will be destroyed when the window is destroyed. (And in fact we can't destroy it manually, since the window handle will be invalid.)
    if unsafe {
        SetCoalescableTimer(
            Some(window),
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
            Some(window),
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
    _ = unsafe { ShowWindow(window, SW_SHOWNA) };

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
        Err(Error::from_hresult(HRESULT(exit_code)))
    }
}
