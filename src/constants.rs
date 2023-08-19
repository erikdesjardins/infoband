use windows::Win32::Foundation::WPARAM;

// Sizing and positioning
pub const UNSCALED_OFFSET_FROM_RIGHT_EDGE: i32 = 300;
pub const UNSCALED_WINDOW_WIDTH: i32 = 200;

// User messages
pub const UM_ENABLE_DEBUG_PAINT: WPARAM = WPARAM(1);
pub const UM_INITIAL_PAINT: WPARAM = WPARAM(2);

// Timer ids
pub const IDT_REDRAW_TIMER: WPARAM = WPARAM(1);

// Timer intervals
pub const REDRAW_TIMER_MS: u32 = 5 * 1000;
