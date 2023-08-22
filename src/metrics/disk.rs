use std::cell::Cell;
use std::mem;
use std::ptr::addr_of_mut;
use std::time::Duration;
use windows::core::{Result, GUID};
use windows::Win32::Foundation::{HANDLE, WIN32_ERROR};
use windows::Win32::System::Performance::{
    PerfAddCounters, PerfCloseQueryHandle, PerfOpenQueryHandle, PerfQueryCounterData,
    PERF_COUNTER_DATA, PERF_COUNTER_HEADER, PERF_COUNTER_IDENTIFIER, PERF_DATA_HEADER,
    PERF_SINGLE_COUNTER, PERF_WILDCARD_COUNTER,
};

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

pub struct State {
    // Handle holding perf query.
    // SAFETY: must not be modified or dropped until this struct is dropped.
    handle: HANDLE,

    // Previous count of total bytes transferred.
    prev_byte_count: Cell<u64>,
}

impl Drop for State {
    fn drop(&mut self) {
        // SAFETY: handle is valid and hasn't been closed due to our safety invariant.
        if let Err(e) = unsafe { WIN32_ERROR(PerfCloseQueryHandle(self.handle)).ok() } {
            log::error!("Failed to close PerfQueryHandle: {}", e);
        }
    }
}

impl State {
    pub fn new() -> Result<Self> {
        // Create handle to hold counters which we will repeatedly query.

        let handle = {
            let mut handle = HANDLE::default();
            // SAFETY: handle is a valid pointer to PerfQueryHandle
            unsafe { WIN32_ERROR(PerfOpenQueryHandle(None, &mut handle)).ok()? };
            handle
        };

        // Add counters to the query handle.

        #[repr(C)]
        #[repr(align(8))]
        struct PERF_COUNTER_IDENTIFIER_WITH_NAME {
            identifier: PERF_COUNTER_IDENTIFIER,
            name_filter: [u16; 7],
            null: u16,
        }

        // Always filter to the "default" instance.
        // From introspection, it seems that there are only two instances: "default" and "_Total", which always have the same value.
        let name_filter = {
            let mut name = [0u16; 7];
            for (c, n) in "default".encode_utf16().zip(&mut name) {
                *n = c;
            }
            name
        };
        let mut counters = [
            // "FileSystem Bytes Read" counter, "default" instance
            PERF_COUNTER_IDENTIFIER_WITH_NAME {
                identifier: PERF_COUNTER_IDENTIFIER {
                    CounterSetGuid: FILESYSTEM_DISK_ACTIVITY_COUNTERSET,
                    Size: mem::size_of::<PERF_COUNTER_IDENTIFIER_WITH_NAME>() as u32,
                    CounterId: FILESYSTEM_BYTES_READ_COUNTER,
                    InstanceId: PERF_WILDCARD_COUNTER,
                    ..Default::default()
                },
                name_filter,
                null: 0,
            },
            // "FileSystem Bytes Written" counter, "default" instance
            PERF_COUNTER_IDENTIFIER_WITH_NAME {
                identifier: PERF_COUNTER_IDENTIFIER {
                    CounterSetGuid: FILESYSTEM_DISK_ACTIVITY_COUNTERSET,
                    Size: mem::size_of::<PERF_COUNTER_IDENTIFIER_WITH_NAME>() as u32,
                    CounterId: FILESYSTEM_BYTES_WRITTEN_COUNTER,
                    InstanceId: PERF_WILDCARD_COUNTER,
                    ..Default::default()
                },
                name_filter,
                null: 0,
            },
        ];
        let counters_size = mem::size_of_val(&counters).try_into().unwrap();

        // SAFETY: handle is valid, counters matches the defined layout for PERF_COUNTER_IDENTIFIER blocks.
        // https://learn.microsoft.com/en-us/windows/win32/api/perflib/ns-perflib-perf_counter_identifier
        unsafe {
            WIN32_ERROR(PerfAddCounters(
                handle,
                counters.as_mut_ptr().cast::<PERF_COUNTER_IDENTIFIER>(),
                counters_size,
            ))
            .ok()?
        };

        // Consume status from adding each identifier.
        for counter in &counters {
            WIN32_ERROR(counter.identifier.Status).ok()?;
        }

        // Populate query indexes for the counters.
        // (For some reason data is not always returned in an order matching the order queries were added.)
        // Commented out because we don't care to distinguish between bytes read and written.

        // unsafe {
        //     WIN32_ERROR(PerfQueryCounterInfo(
        //         handle,
        //         Some(counters.as_mut_ptr().cast::<PERF_COUNTER_IDENTIFIER>()),
        //         counters_size,
        //         &mut 0,
        //     ))
        //     .ok()?
        // };

        Ok(Self {
            handle,
            prev_byte_count: Cell::new(0),
        })
    }

    pub fn fetch_mbyte(&self, time_delta: Option<Duration>) -> Result<f64> {
        // Get data from perf counters.
        // https://learn.microsoft.com/en-us/windows/win32/api/perflib/nf-perflib-perfquerycounterdata

        // Technically, I infer you are supposed to call PerfQueryCounterData first to determine how big of a buffer to allocate,
        // then call it again with the buffer. But since we are only querying for a specific fixed result,
        // make a struct that's exactly the right size, in the hope that it will generate that layout.
        // Just in case, we also check that the fields to ensure they match our expected layout.

        #[derive(Default)]
        #[repr(C)]
        struct PerfDataResults {
            header: PERF_DATA_HEADER,
            counter0: PERF_COUNTER_HEADER,
            counter0_data: PERF_COUNTER_DATA,
            counter0_data_value: u64,
            counter1: PERF_COUNTER_HEADER,
            counter1_data: PERF_COUNTER_DATA,
            counter1_data_value: u64,
        }

        assert_eq!(mem::align_of::<PerfDataResults>(), 8);

        let mut results = PerfDataResults::default();
        let results_size = mem::size_of_val(&results).try_into().unwrap();

        // SAFETY: `handle` is valid; `results` pointer is valid for `writes` for `results_size` bytes
        unsafe {
            WIN32_ERROR(PerfQueryCounterData(
                self.handle,
                Some(addr_of_mut!(results).cast::<PERF_DATA_HEADER>()),
                results_size,
                &mut 0,
            ))
            .ok()?
        };

        assert_eq!(
            results.header.dwNumCounters, 2,
            "header must have two counters"
        );
        assert_eq!(
            results.counter0.dwType, PERF_SINGLE_COUNTER,
            "first counter must be a single counter"
        );
        assert_eq!(
            results.counter1.dwType, PERF_SINGLE_COUNTER,
            "second counter must be a single counter"
        );
        assert_eq!(
            results.counter0_data.dwDataSize, 8,
            "first counter must be a u64 counter"
        );
        assert_eq!(
            results.counter0_data.dwDataSize, 8,
            "second counter must be a u64 counter"
        );

        // Counter ordering is not guaranteed, so this might be read,write or write,read; see above.
        let bytes_read_or_written = results.counter0_data_value;
        let bytes_written_or_read = results.counter1_data_value;

        let cur_byte_count = bytes_read_or_written.wrapping_add(bytes_written_or_read);

        // On first sample, just store the current byte count and return zero.
        let mbyte = match time_delta {
            Some(time_delta) => {
                let bytes = cur_byte_count.wrapping_sub(self.prev_byte_count.get());
                bytes as f64 / (1024 * 1024) as f64 / time_delta.as_secs_f64()
            }
            None => 0.0,
        };

        self.prev_byte_count.set(cur_byte_count);

        Ok(mbyte)
    }
}
