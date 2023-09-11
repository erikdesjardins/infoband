use crate::constants::PDH_FMT_NOSCALE;
use std::time::Duration;
use windows::core::{w, Error, Result, HRESULT, HSTRING};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue,
    PdhOpenQueryW, PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT, PDH_FMT_COUNTERVALUE,
    PDH_FMT_LARGE,
};

pub struct State {
    /// Handle to the query.
    // SAFETY: must be a valid PDH query handle; must not be closed except in this type's destructor.
    query: isize,
    /// Handle to the query counter.
    counter: isize,
}

impl Drop for State {
    fn drop(&mut self) {
        if let Err(e) = unsafe { HRESULT(PdhCloseQuery(self.query) as _).ok() } {
            log::error!("Failed to close PDH query: {}", e);
        }
    }
}

impl State {
    pub fn new() -> Result<Self> {
        let query = {
            let mut query = 0;
            unsafe { HRESULT(PdhOpenQueryW(None, 0, &mut query) as _).ok()? };
            query
        };

        // Create instance right after opening query, so that it gets closed in case of error.
        let mut this = Self { query, counter: 0 };

        // Add counter for total bytes
        let counter = {
            let mut counter = 0;
            unsafe {
                HRESULT(PdhAddEnglishCounterW(
                    query,
                    //w!(r"\Network Interface(*)\Bytes Total/sec"),
                    // TODO: need to use wildcard here, but expand it for all interfaces with PdhExpandCounterPath
                    w!(
                        r"\Network Interface(Realtek PCIe 2.5GbE Family Controller)\Bytes Total/sec"
                    ),
                    0,
                    &mut counter,
                ) as _)
                .ok()?
            };
            counter
        };
        this.counter = counter;

        // Do initial fetch since counter requires it to compute deltas.
        unsafe { HRESULT(PdhCollectQueryData(this.query) as _).ok()? };

        Ok(this)
    }

    pub fn fetch_mbit(&self, time_delta: Option<Duration>) -> Result<f64> {
        unsafe { HRESULT(PdhCollectQueryData(self.query) as _).ok()? };

        let mut counter_value = PDH_FMT_COUNTERVALUE::default();
        unsafe {
            HRESULT(PdhGetFormattedCounterValue(
                self.counter,
                PDH_FMT(PDH_FMT_LARGE.0 | PDH_FMT_NOSCALE.0),
                None,
                &mut counter_value,
            ) as _)
            .ok()?
        };

        if !matches!(
            counter_value.CStatus,
            PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA
        ) {
            return Err(Error::new(
                HRESULT(counter_value.CStatus as _),
                HSTRING::new(),
            ));
        }

        let total_bytes = unsafe { counter_value.Anonymous.largeValue };

        let bits_per_byte = 8;
        let bits = total_bytes * bits_per_byte;
        let mbit = (bits as f64) / 1_000_000.0;

        Ok(mbit)
    }
}
