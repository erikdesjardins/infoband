use std::array;
use std::marker::PhantomData;
use std::mem;
use std::ptr::addr_of_mut;
use windows::Win32::Foundation::{HANDLE, WIN32_ERROR};
use windows::Win32::System::Performance::{
    PERF_COUNTER_DATA, PERF_COUNTER_HEADER, PERF_COUNTER_IDENTIFIER, PERF_DATA_HEADER,
    PERF_SINGLE_COUNTER, PERF_WILDCARD_COUNTER, PerfAddCounters, PerfCloseQueryHandle,
    PerfCounterDataType, PerfOpenQueryHandle, PerfQueryCounterData, PerfQueryCounterInfo,
};
use windows::core::{GUID, Result};

/// Represents the type of data that will be fetched from a performance counter,
/// which impacts the memory layout of the blocks that will be generated by PerfQueryCounterData.
/// Corresponds to the PerfCounterDataType enum.
///
/// Workaround for lack of const generic enum variants.
pub trait PerfCounterType {
    const TYPE: PerfCounterDataType;
}

/// Represents PERF_SINGLE_COUNTER, filtered to a single counter.
pub struct SingleCounter;

impl PerfCounterType for SingleCounter {
    const TYPE: PerfCounterDataType = PERF_SINGLE_COUNTER;
}

/// Represents an open performance query handle.
/// Can be repeatedly queried to get perf data.
pub struct PerfQueries<Type, const COUNTERS: usize, CounterValue>
where
    Type: PerfCounterType,
    CounterValue: Copy + Default,
{
    /// The handle to the performance query.
    // SAFETY: must not be modified or dropped until this struct is dropped.
    handle: HANDLE,
    /// Indexes of the counters in query results (since for some reason this is not guaranteed)
    counter_indexes: [u32; COUNTERS],
    /// The structore of data that will be returned by this perf query (e.g. single or multi value).
    _type: PhantomData<Type>,
    /// The type of data that will be fetched from this handle.
    /// Usually u64 or u32.
    _value: PhantomData<CounterValue>,
}

impl<Type, const COUNTERS: usize, CounterValue> Drop for PerfQueries<Type, COUNTERS, CounterValue>
where
    Type: PerfCounterType,
    CounterValue: Copy + Default,
{
    fn drop(&mut self) {
        // SAFETY: handle is valid and hasn't been closed due to our safety invariant.
        if let Err(e) = unsafe { WIN32_ERROR(PerfCloseQueryHandle(self.handle)).ok() } {
            log::error!("Failed to close PerfQueryHandle: {e}");
        }
    }
}

impl<const COUNTERS: usize, CounterValue> PerfQueries<SingleCounter, COUNTERS, CounterValue>
where
    CounterValue: Copy + Default,
{
    /// Query the given counterset, for the given counter ids, filtered to the given instance name filter.
    ///
    /// Since we expect there to be a single instance, the instance name filter cannot be `b"*"` or `b""`.
    pub fn new_filtered_to_single_counter<const N: usize>(
        counterset: GUID,
        counter_ids: &[u32; COUNTERS],
        instance_name_filter: &[u8; N],
    ) -> Result<Self> {
        assert!(!matches!(instance_name_filter.as_slice(), b"" | b"*"));

        let instance_name_filter = instance_name_filter.map(|c| {
            let mut one_char = [0; 1];
            let c = char::from_u32(u32::from(c))
                .unwrap_or_else(|| panic!("Filter string must be valid UTF-8"));
            c.encode_utf16(&mut one_char);
            one_char[0]
        });

        // Create handle to hold counters which we will repeatedly query.
        let handle = {
            let mut handle = HANDLE::default();
            // SAFETY: handle is a valid pointer to PerfQueryHandle
            unsafe { WIN32_ERROR(PerfOpenQueryHandle(None, &mut handle)).ok()? };
            handle
        };

        // Create instance right after handle so the handle will be dropped if we error.
        let mut queries = PerfQueries {
            handle,
            counter_indexes: [0; COUNTERS], // will be filled in later
            _type: PhantomData,
            _value: PhantomData,
        };

        // Add counters to the query handle.

        #[repr(C)]
        #[repr(align(8))]
        struct PERF_COUNTER_IDENTIFIER_WITH_NAME<const N: usize> {
            identifier: PERF_COUNTER_IDENTIFIER,
            name_filter: [u16; N],
            null: u16,
        }

        let mut counters = counter_ids.map(|counter_id| PERF_COUNTER_IDENTIFIER_WITH_NAME {
            identifier: PERF_COUNTER_IDENTIFIER {
                CounterSetGuid: counterset,
                Size: mem::size_of::<PERF_COUNTER_IDENTIFIER_WITH_NAME<N>>()
                    .try_into()
                    .unwrap(),
                CounterId: counter_id,
                // Note that, per https://learn.microsoft.com/en-us/windows/win32/api/perflib/ns-perflib-perf_instance_header#remarks,
                // each instance is identified by _both_ its instance id and name combined...
                // In practice, I do see duplicate instance IDs frequently, but I don't see duplicate names,
                // so we use the wildcard instance ID here and only filter on the name (below).
                InstanceId: PERF_WILDCARD_COUNTER,
                ..Default::default()
            },
            name_filter: instance_name_filter,
            null: 0,
        });
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

        unsafe {
            WIN32_ERROR(PerfQueryCounterInfo(
                handle,
                Some(counters.as_mut_ptr().cast::<PERF_COUNTER_IDENTIFIER>()),
                counters_size,
                &mut 0,
            ))
            .ok()?
        };

        queries.counter_indexes = counters.map(|counter| counter.identifier.Index);

        Ok(queries)
    }

    /// Query data from perf counters.
    pub fn query_data(&self) -> Result<[CounterValue; COUNTERS]> {
        // Get data from perf counters.
        // https://learn.microsoft.com/en-us/windows/win32/api/perflib/nf-perflib-perfquerycounterdata

        // Technically, I infer you are supposed to call PerfQueryCounterData first to determine how big of a buffer to allocate,
        // then call it again with the buffer. But since we are only querying for a specific fixed result,
        // make a struct that's exactly the right size, in the hope that it will generate that layout.
        // Just in case, we also check that the fields to ensure they match our expected layout.

        #[repr(C)]
        #[repr(align(8))]
        struct PerfDataResults<const COUNTERS: usize, CounterValue> {
            header: PERF_DATA_HEADER,
            counters: [PerfDataCounter<CounterValue>; COUNTERS],
        }

        #[derive(Default)]
        #[repr(C)]
        struct PerfDataCounter<CounterValue> {
            header: PERF_COUNTER_HEADER,
            data_prefix: PERF_COUNTER_DATA,
            value: CounterValue,
        }

        let mut results = PerfDataResults::<COUNTERS, _> {
            header: Default::default(),
            counters: array::from_fn(|_| Default::default()),
        };
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
            results.header.dwNumCounters, COUNTERS as u32,
            "must have the correct number of counters"
        );

        for (i, counter) in results.counters.iter().enumerate() {
            // Consume status from counter fetch
            WIN32_ERROR(counter.header.dwStatus).ok()?;

            assert_eq!(
                counter.header.dwType,
                SingleCounter::TYPE,
                "counter {i} must have correct type"
            );
            assert_eq!(
                counter.data_prefix.dwDataSize,
                mem::size_of::<CounterValue>() as u32,
                "data size must be valid"
            );
        }

        let values = self
            .counter_indexes
            .map(|i| results.counters[i as usize].value);

        Ok(values)
    }
}
