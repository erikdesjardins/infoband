use std::array;
use std::cell::Cell;
use std::ops::{Add, Mul, Sub};

pub struct CircularBuffer<T, const N: usize>
where
    T: Default + Copy,
{
    samples: [Cell<T>; N],
    next_index: Cell<usize>,
    len: Cell<usize>,
}

impl<T, const N: usize> Default for CircularBuffer<T, N>
where
    T: Default + Copy,
{
    fn default() -> Self {
        Self {
            samples: array::from_fn(|_| Cell::new(T::default())),
            next_index: Cell::new(0),
            len: Cell::new(0),
        }
    }
}

impl<T, const N: usize> CircularBuffer<T, N>
where
    T: Default + Copy,
{
    pub fn push(&self, sample: T) {
        let index = self.next_index.get();
        self.samples[index].set(sample);
        self.next_index.set((index + 1) % N);
        self.len.set((self.len.get() + 1).min(N));
    }

    pub fn exponential_moving_average(&self, alpha: f64) -> T
    where
        T: From<f64> + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
    {
        assert!(alpha > 0.0 && alpha <= 1.0);
        let mut result = T::default();
        let mut weight = 1.0;
        let mut index = self.next_index.get();
        for _ in 0..self.len.get() {
            index = (index + N - 1) % N;
            let sample = self.samples[index].get();
            result = result + T::from(weight) * (sample - result);
            weight *= alpha;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circular_buffer() {
        let buffer = CircularBuffer::<f64, 3>::default();
        assert_eq!(buffer.exponential_moving_average(0.5), 0.0);
        buffer.push(1.0);
        assert_eq!(buffer.exponential_moving_average(0.5), 1.0);
        buffer.push(2.0);
        assert_eq!(buffer.exponential_moving_average(0.5), 1.5);
        buffer.push(3.0);
        assert_eq!(buffer.exponential_moving_average(0.5), 2.125);
        buffer.push(4.0);
        assert_eq!(buffer.exponential_moving_average(0.5), 3.125);
        buffer.push(5.0);
        assert_eq!(buffer.exponential_moving_average(0.5), 4.125);
    }
}
