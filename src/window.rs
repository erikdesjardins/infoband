use crate::constants::{
    HOTKEY_MIC_MUTE, UM_ENABLE_DEBUG_PAINT, UM_ENABLE_KEEP_AWAKE, UM_INITIAL_METRICS,
    UM_INITIAL_MIC_STATE, UM_INITIAL_PAINT, UM_INITIAL_Z_ORDER, UM_SET_OFFSET_FROM_RIGHT,
};
use crate::defer;
use crate::opt::MicrophoneHotkey;
use crate::utils::Unscaled;
use crate::window::proc::window_proc;
use windows::Win32::Foundation::{HINSTANCE, LPARAM};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DispatchMessageW, GetMessageW,
    IDC_ARROW, LoadCursorW, MSG, PostMessageW, RegisterClassW, RegisterShellHookWindow, SW_SHOWNA,
    ShowWindow, WM_USER, WNDCLASSW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{Error, HRESULT, Result, w};

mod awake;
mod messages;
mod microphone;
mod paint;
mod proc;
mod state;
mod timers;
mod z_order;

/// Create the toplevel window, start timers for updating it, and pump the windows message loop.
pub fn create_and_run_message_loop(
    offset_from_right: Unscaled<i32>,
    mic_hotkey: Option<MicrophoneHotkey>,
    keep_awake_while_unlocked: bool,
    debug_paint: bool,
) -> Result<()> {
    // Initialize COM, to be used by the microphone management code.
    // Ideally, we would put this in the microphone state code, but the docs suggest that:
    // > CoUninitialize should be called on application shutdown, as the last call made to the COM library
    // > after the application hides its main windows and falls through its main message loop.
    // https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-couninitialize
    // ...so it's not clear that uninitializing when the state is dropped is correct.
    // Thus, since we have to uninitialize in this function anyways, we also initialize here for consistency.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()? };
    defer! {
        unsafe { CoUninitialize() };
    };

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

    // Register hotkey for mic muting.
    if let Some(mic_hotkey) = &mic_hotkey {
        let modifiers = {
            // Always forbid repeat, and add other modifiers as necessary.
            let mut modifiers = MOD_NOREPEAT;
            if mic_hotkey.win {
                modifiers |= MOD_WIN;
            }
            if mic_hotkey.shift {
                modifiers |= MOD_SHIFT;
            }
            if mic_hotkey.ctrl {
                modifiers |= MOD_CONTROL;
            }
            if mic_hotkey.alt {
                modifiers |= MOD_ALT;
            }
            modifiers
        };
        unsafe {
            RegisterHotKey(
                Some(window),
                HOTKEY_MIC_MUTE.0.try_into().unwrap(),
                modifiers,
                u32::from(mic_hotkey.virtual_key_code),
            )?
        };
    }

    // Enqueue a message to tell the window to stay awake
    if keep_awake_while_unlocked {
        unsafe { PostMessageW(Some(window), WM_USER, UM_ENABLE_KEEP_AWAKE, LPARAM(0))? };
    }

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

    // Enqueue a message for initial mic state update
    if mic_hotkey.is_some() {
        unsafe { PostMessageW(Some(window), WM_USER, UM_INITIAL_MIC_STATE, LPARAM(0))? };
    }

    // Enqueue a message for initial z-order update
    unsafe { PostMessageW(Some(window), WM_USER, UM_INITIAL_Z_ORDER, LPARAM(0))? };

    // Enqueue a message for initial paint
    unsafe { PostMessageW(Some(window), WM_USER, UM_INITIAL_PAINT, LPARAM(0))? };

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
