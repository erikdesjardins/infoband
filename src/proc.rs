use std::ptr::NonNull;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GetWindowLongPtrW, SetWindowLongPtrW, GWLP_USERDATA, WM_NCCREATE, WM_NCDESTROY,
};

pub trait ProcHandler: Default + Sync {
    /// Handle a window message.
    ///
    /// If this returns `None`, the message will be passed to `DefWindowProcW`.
    fn handle(&self, window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM)
        -> Option<LRESULT>;
}

pub unsafe extern "system" fn window_proc<H: ProcHandler>(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state = match message {
        // On create, set to default state
        WM_NCCREATE => {
            let state = Box::<H>::default();
            // SAFETY: handle is valid as we created it
            unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, Box::into_raw(state) as isize) };
            // SAFETY: propagates same safety requirements as caller
            return unsafe { DefWindowProcW(window, message, wparam, lparam) };
        }
        // On destroy, drop state
        WM_NCDESTROY => {
            // SAFETY: setting state to 0 is always safe; type will be valid since we set it
            let state = unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, 0) as *mut H };
            if !state.is_null() {
                // SAFETY: state is either valid (as we set GWLP_USERDATA when constructing the window), or null
                unsafe { drop(Box::from_raw(state)) };
            }
            // SAFETY: propagates same safety requirements as caller
            return unsafe { DefWindowProcW(window, message, wparam, lparam) };
        }
        // For all other messages, get the state and handle as normal...
        _ => {
            // SAFETY: state is valid or null
            let state = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *mut H };
            let state = NonNull::new(state);
            // SAFETY: state is either valid (as we set GWLP_USERDATA when constructing the window), or null
            unsafe { state.map(|s| s.as_ref()) }
        }
    };

    let Some(state) = state else {
        log::warn!(
            "Window proc invoked with no state set (message=0x{:04x}, wparam=0x{:016x} lparam=0x{:016x})",
            message,
            wparam.0,
            lparam.0
        );
        // SAFETY: propagates same safety requirements as caller
        return unsafe { DefWindowProcW(window, message, wparam, lparam) };
    };

    let Some(result) = state.handle(window, message, wparam, lparam) else {
        // SAFETY: propagates same safety requirements as caller
        return unsafe { DefWindowProcW(window, message, wparam, lparam) };
    };

    result
}
