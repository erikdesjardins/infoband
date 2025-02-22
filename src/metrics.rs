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
        let time = Instant::now();
        let prev_time = self.prev_time.replace(Some(time));
        let time_delta = prev_time.map(|prev_time| time - prev_time);

        match self.cpu.fetch_percent() {
            Ok(cpu) => {
                log::trace!("Fetched CPU: {cpu:.3}");
                self.cpu_percent.push(cpu);
            }
            Err(e) => log::error!("Failed to fetch CPU: {e}"),
        }

        match self.memory.fetch_percent() {
            Ok(memory) => {
                log::trace!("Fetched memory: {memory:.3}");
                self.memory_percent.push(memory);
            }
            Err(e) => log::error!("Failed to fetch memory: {e}"),
        }

        match self.disk.fetch_mbyte(time_delta) {
            Ok(disk) => {
                log::trace!("Fetched disk: {disk:.3}");
                self.disk_mbyte.push(disk);
            }
            Err(e) => log::error!("Failed to fetch disk: {e}"),
        }

        match self.network.fetch_mbit(time_delta) {
            Ok(network) => {
                log::trace!("Fetched network: {network:.3}");
                self.network_mbit.push(network);
            }
            Err(e) => log::error!("Failed to fetch network: {e}"),
        }
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
