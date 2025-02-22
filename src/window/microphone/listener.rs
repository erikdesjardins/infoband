use crate::constants::{IDT_MIC_STATE_TIMER, MIC_STATE_TIMER_COALESCE, MIC_STATE_TIMER_MS};
use crate::defer;
use windows::core::Result;
use windows::Win32::Foundation::HWND;
use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::{
    eCapture, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::WindowsAndMessaging::SetCoalescableTimer;
use windows_core::{implement, Error, HSTRING};

pub struct ListenerManager {
    dev_enumerator: IMMDeviceEnumerator,
    listener: IAudioEndpointVolumeCallback,
    // List of all endpoints and their corresponding IDs.
    //
    // Invariant: All items in the list must have been registered with the `listener` via `RegisterControlChangeNotify`.`
    registered_endpoints: Vec<(HSTRING, IAudioEndpointVolume)>,
}

impl Drop for ListenerManager {
    fn drop(&mut self) {
        for (id, endpoint) in &self.registered_endpoints {
            if let Err(e) = unsafe { endpoint.UnregisterControlChangeNotify(&self.listener) } {
                log::error!("Unregistering listener failed for {}: {}", id, e);
            }
        }
    }
}

impl ListenerManager {
    pub fn new(window: HWND) -> Result<Self> {
        let dev_enumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)? };

        Ok(Self {
            dev_enumerator,
            listener: IAudioEndpointVolumeCallback::from(MicrophoneListener { window }),
            registered_endpoints: Vec::new(),
        })
    }

    pub fn endpoints(&self) -> impl Iterator<Item = &IAudioEndpointVolume> {
        self.registered_endpoints
            .iter()
            .map(|(_, endpoint)| endpoint)
    }

    pub fn refresh_endpoints(&mut self) -> Result<()> {
        let endpoints = unsafe {
            self.dev_enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)?
        };

        let count = unsafe { endpoints.GetCount()? };

        for i in 0..count {
            // Get the device...
            let device = unsafe { endpoints.Item(i)? };

            // ...and its ID.
            let id = unsafe { device.GetId()? };
            defer! {
                unsafe { CoTaskMemFree(Some(id.as_ptr().cast())) };
            }
            assert!(!id.is_null());
            let id_bytes = unsafe { id.as_wide() };

            // Ignore devices we've already registered.
            if self.registered_endpoints.iter().any(|(i, _)| &**i == id_bytes) {
                continue;
            }

            let id = unsafe { id.to_hstring() };

            // If not already registered, get the endpoint...
            let endpoint =
                unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_INPROC_SERVER, None)? };

            // ...and register the listener.
            unsafe { endpoint.RegisterControlChangeNotify(Some(&self.listener))? };

            log::debug!("Registered listener for mic {}", id);

            // Invariant: we just successfully registered the listener above.
            self.registered_endpoints.push((id, endpoint));
        }

        Ok(())
    }
}

#[implement(IAudioEndpointVolumeCallback)]
struct MicrophoneListener {
    window: HWND,
}

impl IAudioEndpointVolumeCallback_Impl for MicrophoneListener_Impl {
    fn OnNotify(
        &self,
        _: *mut windows::Win32::Media::Audio::AUDIO_VOLUME_NOTIFICATION_DATA,
    ) -> Result<()> {
        // WARNING: this may be called from another thread, so we can only do thread-safe operations here.

        // If there is an existing timer running, this will overwrite it, producing a debounce effect.
        if unsafe {
            SetCoalescableTimer(
                Some(self.window),
                IDT_MIC_STATE_TIMER.0,
                MIC_STATE_TIMER_MS,
                None,
                MIC_STATE_TIMER_COALESCE,
            )
        } == 0
        {
            return Err(Error::from_win32());
        }

        Ok(())
    }
}
