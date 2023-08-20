use crate::constants::{EXPONENTIAL_DECAY_ALPHA, SAMPLE_COUNT};
use crate::stats::CircularBuffer;

#[derive(Default)]
pub struct Metrics {
    cpu_percent: CircularBuffer<f64, SAMPLE_COUNT>,
    memory_percent: CircularBuffer<f64, SAMPLE_COUNT>,
    disk_mbyte: CircularBuffer<f64, SAMPLE_COUNT>,
    network_mbit: CircularBuffer<f64, SAMPLE_COUNT>,
}

impl Metrics {
    pub fn fetch(&self) {
        self.fetch_cpu();
        self.fetch_memory();
        self.fetch_disk();
        self.fetch_network();
    }

    fn fetch_cpu(&self) {
        // TODO: implement
        self.cpu_percent.push(0.0);
    }

    fn fetch_memory(&self) {
        // TODO: implement
        self.memory_percent.push(0.0);
    }

    fn fetch_disk(&self) {
        // TODO: implement
        self.disk_mbyte.push(0.0);
    }

    fn fetch_network(&self) {
        // TODO: implement
        self.network_mbit.push(0.0);
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
