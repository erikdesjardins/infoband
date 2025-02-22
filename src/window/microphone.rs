use listener::ListenerManager;
use std::cell::{Cell, RefCell};
use std::ptr;
use windows::core::Result;
use windows::Win32::Foundation::HWND;

mod listener;

pub struct Microphone {
    listener: RefCell<ListenerManager>,
    is_muted: Cell<bool>,
}

impl Microphone {
    pub fn new(window: HWND) -> Result<Self> {
        Ok(Self {
            listener: RefCell::new(ListenerManager::new(window)?),
            // Assume muted in initial state, to avoid showing the microphone warning banner on startup.
            is_muted: Cell::new(true),
        })
    }

    pub fn is_muted(&self) -> bool {
        self.is_muted.get()
    }

    pub fn kill_timer(&self, window: HWND) {
        self.listener.borrow().kill_timer(window);
    }

    pub fn refresh_devices(&self) {
        if let Err(e) = self.refresh_devices_fallible() {
            log::error!("Refreshing active microphones failed: {}", e);
        }
    }

    fn refresh_devices_fallible(&self) -> Result<()> {
        self.listener.borrow_mut().refresh_endpoints()?;

        Ok(())
    }

    pub fn update_muted_state(&self) {
        if let Err(e) = self.update_muted_state_fallible() {
            log::error!("Updating muted state failed: {}", e);
        }
    }

    fn update_muted_state_fallible(&self) -> Result<()> {
        let mut all_muted = true;

        for endpoint in self.listener.borrow().endpoints() {
            let mute = unsafe { endpoint.GetMute()? };
            if !mute.as_bool() {
                all_muted = false;
                break;
            }
        }

        self.is_muted.set(all_muted);

        Ok(())
    }

    pub fn set_mute(&self, mute: bool) {
        if let Err(e) = self.set_mute_fallible(mute) {
            log::error!("Setting muted state failed: {}", e);
        }
    }

    fn set_mute_fallible(&self, mute: bool) -> Result<()> {
        for endpoint in self.listener.borrow().endpoints() {
            unsafe { endpoint.SetMute(mute, ptr::null())? };
        }

        Ok(())
    }
}
