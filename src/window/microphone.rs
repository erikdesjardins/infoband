use std::cell::Cell;
use std::ptr;
use windows::core::Result;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};

pub struct Microphone {
    dev_enumerator: IMMDeviceEnumerator,
    is_muted: Cell<bool>,
}

impl Microphone {
    pub fn new() -> Result<Self> {
        let dev_enumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)? };

        Ok(Self {
            dev_enumerator,
            // Assume muted in initial state, to avoid showing the microphone warning banner on startup.
            is_muted: Cell::new(true),
        })
    }

    pub fn is_muted(&self) -> bool {
        self.is_muted.get()
    }

    pub fn update_muted_state(&self) {
        if let Err(e) = self.update_muted_state_fallible() {
            log::error!("Updating muted state failed: {}", e);
        }
    }

    fn update_muted_state_fallible(&self) -> Result<()> {
        let endpoints = self.get_all_active_devices()?;

        let mut all_muted = true;

        for endpoint in endpoints {
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
        let endpoints = self.get_all_active_devices()?;

        let mut all_muted = true;

        for endpoint in endpoints {
            unsafe { endpoint.SetMute(mute, ptr::null())? };
            if !mute {
                all_muted = false;
            }
        }

        self.is_muted.set(all_muted);

        Ok(())
    }

    fn get_all_active_devices(&self) -> Result<Vec<IAudioEndpointVolume>> {
        let endpoints = unsafe {
            self.dev_enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)?
        };

        let count = unsafe { endpoints.GetCount()? };

        (0..count)
            .map(|i| {
                let device = unsafe { endpoints.Item(i)? };
                let endpoint =
                    unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_INPROC_SERVER, None)? };
                Ok(endpoint)
            })
            .collect()
    }
}
