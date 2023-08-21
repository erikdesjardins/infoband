use crate::constants::{EXPONENTIAL_DECAY_ALPHA, SAMPLE_COUNT};
use crate::stats::CircularBuffer;
use memoffset::offset_of;
use std::cell::Cell;
use std::mem;
use std::ptr::addr_of_mut;
use std::time::{Duration, Instant};
use windows::core::Result;
use windows::Win32::Foundation::{FILETIME, WIN32_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{GetIfTable, MIB_IFROW, MIB_IFTABLE};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

#[derive(Default)]
pub struct Metrics {
    /// Timestamp of the last time metrics were fetched.
    prev_time: Cell<Option<Instant>>,

    // Previous cumulative durations of CPU time.
    prev_cpu_times: Cell<Option<(u64, u64, u64)>>,
    /// Samples of CPU usage as a percentage of total CPU time.
    cpu_percent: CircularBuffer<f64, SAMPLE_COUNT>,

    /// Samples of memory usage as a percentage of total memory.
    memory_percent: CircularBuffer<f64, SAMPLE_COUNT>,

    /// Samples of disk bandwidth in megabytes per second.
    disk_mbyte: CircularBuffer<f64, SAMPLE_COUNT>,

    /// Count of total bytes transferred from the previous fetch.
    prev_network_byte_count: Cell<u64>,
    /// Samples of network bandwidth in megabits per second.
    network_mbit: CircularBuffer<f64, SAMPLE_COUNT>,
}

impl Metrics {
    #[inline(never)]
    pub fn fetch(&self) {
        if let Err(e) = self.fetch_fallible() {
            log::error!("Metrics fetch failed: {}", e);
        }
    }

    fn fetch_fallible(&self) -> Result<()> {
        let cur_time = Instant::now();
        let prev_time = self.prev_time.replace(Some(cur_time));
        let time_delta = prev_time.map(|prev_time| cur_time - prev_time);

        let cpu = fetch_cpu(&self.prev_cpu_times)?;
        let memory = fetch_memory()?;
        let disk = fetch_disk()?;
        let network = fetch_network(time_delta, &self.prev_network_byte_count)?;

        log::debug!(
            "Fetched samples: cpu={:.3} memory={:.3} disk={:.3} network={:.3}",
            cpu,
            memory,
            disk,
            network
        );

        self.cpu_percent.push(cpu);
        self.memory_percent.push(memory);
        self.disk_mbyte.push(disk);
        self.network_mbit.push(network);

        Ok(())
    }

    pub fn avg_cpu_percent(&self) -> f64 {
        self.cpu_percent
            .exponential_moving_average(EXPONENTIAL_DECAY_ALPHA)
    }

    pub fn avg_memory_percent(&self) -> f64 {
        self.memory_percent
            .exponential_moving_average(EXPONENTIAL_DECAY_ALPHA)
    }

    pub fn avg_disk_mbyte(&self) -> f64 {
        self.disk_mbyte
            .exponential_moving_average(EXPONENTIAL_DECAY_ALPHA)
    }

    pub fn avg_network_mbit(&self) -> f64 {
        self.network_mbit
            .exponential_moving_average(EXPONENTIAL_DECAY_ALPHA)
    }
}

fn fetch_cpu(prev_cpu_times: &Cell<Option<(u64, u64, u64)>>) -> Result<f64> {
    let mut idle = FILETIME::default();
    let mut kernel_plus_idle = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: all pointers are to valid `FILETIME`s
    unsafe {
        GetSystemTimes(
            Some(&mut idle),
            Some(&mut kernel_plus_idle),
            Some(&mut user),
        )
        .ok()?
    };

    let to_100ns_intervals = |filetime: FILETIME| {
        u64::from(filetime.dwHighDateTime) << 32 | u64::from(filetime.dwLowDateTime)
    };

    let cur_idle = to_100ns_intervals(idle);
    let cur_kernel_plus_idle = to_100ns_intervals(kernel_plus_idle);
    let cur_user = to_100ns_intervals(user);

    // On first sample, just store the current times and return zero.
    let percent = match prev_cpu_times.get() {
        Some((prev_idle, prev_kernel_plus_idle, prev_user)) => {
            let idle = cur_idle.wrapping_sub(prev_idle);
            let kernel_plus_idle = cur_kernel_plus_idle.wrapping_sub(prev_kernel_plus_idle);
            let user = cur_user.wrapping_sub(prev_user);

            let time_delta = kernel_plus_idle + user;
            let active_delta = time_delta - idle;

            (active_delta * 100) as f64 / (time_delta as f64)
        }
        None => 0.0,
    };

    prev_cpu_times.set(Some((cur_idle, cur_kernel_plus_idle, cur_user)));

    Ok(percent)
}

fn fetch_memory() -> Result<f64> {
    let mut mem_status = MEMORYSTATUSEX {
        dwLength: mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `mem_status` is a valid `MEMORYSTATUSEX`
    unsafe { GlobalMemoryStatusEx(&mut mem_status).ok()? };

    let used = mem_status.ullTotalPhys - mem_status.ullAvailPhys;
    let total = mem_status.ullTotalPhys;
    let percent = (used * 100) as f64 / total as f64;

    Ok(percent)
}

fn fetch_disk() -> Result<f64> {
    // TODO: implement
    let mbyte = 0.0;
    Ok(mbyte)
}

fn fetch_network(time_delta: Option<Duration>, prev_network_byte_count: &Cell<u64>) -> Result<f64> {
    /// Identical to MIB_IFTABLE but with more rows.
    #[repr(C)]
    struct BIG_MIB_IFTABLE {
        dw_num_entries: u32,
        table: [MIB_IFROW; 128],
    }
    assert!(mem::align_of::<BIG_MIB_IFTABLE>() == mem::align_of::<MIB_IFTABLE>());
    assert!(offset_of!(BIG_MIB_IFTABLE, dw_num_entries) == offset_of!(MIB_IFTABLE, dwNumEntries));
    assert!(offset_of!(BIG_MIB_IFTABLE, table) == offset_of!(MIB_IFTABLE, table));

    // SAFETY: MIB_IFTABLE can be safely zero-initialized
    let mut interfaces: BIG_MIB_IFTABLE = unsafe { mem::zeroed() };
    let mut size_of_interfaces = mem::size_of_val(&interfaces).try_into().unwrap();

    // SAFETY: BIG_MIB_IFTABLE is layout-compatible with MIB_IFTABLE, but with a larger table
    unsafe {
        WIN32_ERROR(GetIfTable(
            Some(addr_of_mut!(interfaces).cast::<MIB_IFTABLE>()),
            &mut size_of_interfaces,
            false,
        ))
        .ok()?
    };

    let interfaces = &mut interfaces.table[..interfaces.dw_num_entries as usize];

    // Windows has many internal copies of the same interface, which results in double-counting.
    //
    // For example:
    // status=INTERNAL_IF_OPER_STATUS(5) type=6 addr=[4, 217, 245, 51, 50, 182, 0, 0] bytes=2288317722 - \DEVICE\TCPIP_{438B8BC2-XXXX-XXXX-XXXX-XXXXXXXXXXXX} Realtek PCIe 2.5GbE Family Controller-WFP Native MAC Layer LightWeight Filter-0000
    // status=INTERNAL_IF_OPER_STATUS(5) type=6 addr=[4, 217, 245, 51, 50, 182, 0, 0] bytes=2288317722 - \DEVICE\TCPIP_{8C3238C4-XXXX-XXXX-XXXX-XXXXXXXXXXXX} Realtek PCIe 2.5GbE Family Controller-Npcap Packet Driver (NPCAP)-0000
    // status=INTERNAL_IF_OPER_STATUS(5) type=6 addr=[4, 217, 245, 51, 50, 182, 0, 0] bytes=2288317722 - \DEVICE\TCPIP_{438B8BC4-XXXX-XXXX-XXXX-XXXXXXXXXXXX} Realtek PCIe 2.5GbE Family Controller-QoS Packet Scheduler-0000
    // status=INTERNAL_IF_OPER_STATUS(5) type=6 addr=[4, 217, 245, 51, 50, 182, 0, 0] bytes=2288317722 - \DEVICE\TCPIP_{438B8BC7-XXXX-XXXX-XXXX-XXXXXXXXXXXX} Realtek PCIe 2.5GbE Family Controller-WFP 802.3 MAC Layer LightWeight Filter-0000
    //
    // To avoid this, deduplicate interfaces by address.

    interfaces.sort_unstable_by_key(|if_row| if_row.bPhysAddr);

    let mut cur_network_byte_count = 0;

    let mut last_address = Default::default();
    for if_row in interfaces {
        if if_row.bPhysAddr == last_address {
            continue;
        }
        last_address = if_row.bPhysAddr;
        cur_network_byte_count += u64::from(if_row.dwInOctets) + u64::from(if_row.dwOutOctets);
    }

    // On first sample, just store the current byte count and return zero.
    let mbit = match time_delta {
        Some(time_delta) => {
            let bits_per_byte = 8;
            let bits =
                cur_network_byte_count.wrapping_sub(prev_network_byte_count.get()) * bits_per_byte;
            (bits as f64) / 1_000_000.0 / time_delta.as_secs_f64()
        }
        None => 0.0,
    };

    prev_network_byte_count.set(cur_network_byte_count);

    Ok(mbit)
}
