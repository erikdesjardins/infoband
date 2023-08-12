use std::sync::atomic::{AtomicUsize, Ordering};

static REFCOUNT: AtomicUsize = AtomicUsize::new(0);

pub fn is_zero() -> bool {
    REFCOUNT.load(Ordering::SeqCst) == 0
}

pub fn increment() {
    let old_value = REFCOUNT.fetch_add(1, Ordering::SeqCst);
    if old_value >= usize::MAX / 2 {
        panic!("refcount overflow (positive)");
    }
}

pub fn decrement() {
    let old_value = REFCOUNT.fetch_sub(1, Ordering::SeqCst);
    if old_value == 0 {
        panic!("refcount overflow (negative)");
    }
}
