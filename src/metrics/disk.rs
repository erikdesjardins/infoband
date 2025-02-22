use crate::perf::{PerfQueries, SingleCounter};
use std::cell::Cell;
use std::time::Duration;
use windows::core::{GUID, Result};

// Should be the counterset ID of "FileSystem Disk Activity", aka {F596750D-B109-4247-A62F-DEA47A46E505}.
// This is a counterset of type PERF_COUNTERSET_MULTI_AGGREGATE.
// If this gets changed at some point, we'll need to use PerfEnumerateCounterSet + PerfQueryCounterSetRegistrationInfo
// to find the ID dynamically, as described in:
// https://learn.microsoft.com/en-ca/windows/win32/perfctrs/using-the-perflib-functions-to-consume-counter-data
const FILESYSTEM_DISK_ACTIVITY_COUNTERSET: GUID =
    GUID::from_u128(0xF596750DB1094247A62FDEA47A46E505);

// "FileSystem Bytes Read" counter ID
const FILESYSTEM_BYTES_READ_COUNTER: u32 = 0;
// "FileSystem Bytes Written" counter ID
const FILESYSTEM_BYTES_WRITTEN_COUNTER: u32 = 1;

// Always filter to the "default" instance.
// From introspection, it seems that there are only two instances: "default" and "_Total", which always have the same value.
const FILESYSTEM_INSTANCE_NAME: &[u8; 6] = b"_Total";

pub struct State {
    queries: PerfQueries<SingleCounter, 2, u64>,
    prev_bytes_read: Cell<u64>,
    prev_bytes_written: Cell<u64>,
}

impl State {
    pub fn new() -> Result<Self> {
        Ok(Self {
            queries: PerfQueries::new_filtered_to_single_counter(
                FILESYSTEM_DISK_ACTIVITY_COUNTERSET,
                &[
                    FILESYSTEM_BYTES_READ_COUNTER,
                    FILESYSTEM_BYTES_WRITTEN_COUNTER,
                ],
                FILESYSTEM_INSTANCE_NAME,
            )?,
            prev_bytes_read: Default::default(),
            prev_bytes_written: Default::default(),
        })
    }

    pub fn fetch_mbyte(&self, time_delta: Option<Duration>) -> Result<f64> {
        let [bytes_read, bytes_written] = self.queries.query_data()?;

        // On first sample, just store the current byte count and return zero.
        let mbyte = match time_delta {
            Some(time_delta) => {
                let bytes_read_delta = bytes_read.wrapping_sub(self.prev_bytes_read.get());
                let bytes_written_delta = bytes_written.wrapping_sub(self.prev_bytes_written.get());

                let total_byte_delta = bytes_read_delta + bytes_written_delta;

                total_byte_delta as f64 / (1024 * 1024) as f64 / time_delta.as_secs_f64()
            }
            None => 0.0,
        };

        self.prev_bytes_read.set(bytes_read);
        self.prev_bytes_written.set(bytes_written);

        Ok(mbyte)
    }
}
