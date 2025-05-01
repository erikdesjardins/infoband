use std::cell::Cell;
use windows::Win32::System::Power::{
    ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, EXECUTION_STATE,
    SetThreadExecutionState,
};
use windows::core::{Error, Result};

pub struct Awake {
    enabled: Cell<bool>,
    currently_kept_awake: Cell<bool>,
}

impl Drop for Awake {
    fn drop(&mut self) {
        _ = self.keep_awake_fallible(false);
    }
}

impl Awake {
    pub fn new() -> Self {
        Self {
            enabled: Cell::new(false),
            currently_kept_awake: Cell::new(false),
        }
    }

    pub fn keep_awake(&self, awake: bool) {
        if let Err(e) = self.keep_awake_fallible(awake) {
            log::error!("Failed to set keep awake state to {awake}: {e}");
        }
    }

    fn keep_awake_fallible(&self, awake: bool) -> Result<()> {
        if !self.enabled.get() {
            return Ok(());
        }
        if self.currently_kept_awake.get() == awake {
            return Ok(());
        }

        let new_state = if awake {
            ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
        } else {
            ES_CONTINUOUS
        };

        // SAFETY: No preconditions.
        let res = unsafe { SetThreadExecutionState(new_state) };
        if res == EXECUTION_STATE(0) {
            return Err(Error::from_win32());
        }

        self.currently_kept_awake.set(awake);
        Ok(())
    }
}
