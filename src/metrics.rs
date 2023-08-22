use crate::constants::{EXPONENTIAL_DECAY_ALPHA, SAMPLE_COUNT};
use crate::stats::CircularBuffer;
use std::cell::Cell;
use std::time::Instant;
use windows::core::Result;

mod cpu;
mod disk;
mod memory;
mod network;

pub struct Metrics {
    /// Timestamp of the last time metrics were fetched.
    prev_time: Cell<Option<Instant>>,

    cpu: cpu::State,
    /// Samples of CPU usage as a percentage of total CPU time.
    cpu_percent: CircularBuffer<f64, SAMPLE_COUNT>,

    memory: memory::State,
    /// Samples of memory usage as a percentage of total memory.
    memory_percent: CircularBuffer<f64, SAMPLE_COUNT>,

    disk: disk::State,
    /// Samples of disk bandwidth in megabytes per second.
    disk_mbyte: CircularBuffer<f64, SAMPLE_COUNT>,

    /// Count of network bytes transferred at the time of the previous fetch.
    network: network::State,
    /// Samples of network bandwidth in megabits per second.
    network_mbit: CircularBuffer<f64, SAMPLE_COUNT>,
}

impl Metrics {
    pub fn new() -> Result<Self> {
        Ok(Self {
            prev_time: Default::default(),
            cpu: Default::default(),
            cpu_percent: Default::default(),
            memory: Default::default(),
            memory_percent: Default::default(),
            disk: disk::State::new()?,
            disk_mbyte: Default::default(),
            network: Default::default(),
            network_mbit: Default::default(),
        })
    }

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

        let cpu = self.cpu.fetch_percent()?;
        let memory = self.memory.fetch_percent()?;
        let disk = self.disk.fetch_mbyte(time_delta)?;
        let network = self.network.fetch_mbit(time_delta)?;

        log::trace!(
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
