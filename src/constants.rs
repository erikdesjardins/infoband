use crate::utils::Unscaled;
use windows::Win32::Foundation::WPARAM;
use windows::Win32::UI::WindowsAndMessaging::TIMERV_DEFAULT_COALESCING;

// Sizing and positioning
//
// Replicating the exact positioning that Windows uses is difficult.
// The constants (far) below result in +/- 1 pixel differences at 100%, 150% (perfect), and 200%, when positioning based on the midpoint of the text.
// Positioning based on the top of the text doesn't seem to work, nor does the bottom.
// I suspect windows is doing something with font metrics and positioning based on the baseline of the text.
//
// As follows are the raw (non-DPI-scaled) offsets to the TOP (not midpoint) of the RENDERING RECT that result in perfect alignment:
// @ 200%
//  ^     ^      ^
//  |     |     14px
//  |    46px    |
//  |     |    first
// 96px   |
//  |  second
//  v
// @ 150%
//  ^     ^      ^
//  |     |     10px
//  |    34px    |
//  |     |    first
// 72px   |
//  |  second
//  v
// @ 100%
//  ^     ^      ^
//  |     |     8px ---> font size 1px too big, so 9px with 1px smaller font
//  |    24px ---|-----> same, would be 25px with 1px smaller font
//  |     |    first
// 48px   |
//  |  second
//  v
//
// As follows are the raw (non-DPI-scaled) offsets to the top and bottom PIXELS (NOT rendering rect) of Windows' text:
// @ 200%
//  ^     ^      ^
//  |     |     24px
//  |    56px    |
//  |     |    first ---> font size 17px
// 96px   |      |
//  |   second   |
//  |     |     55px
//  |    23px    |
//  v     v      v
// @ 150%
//  ^     ^      ^
//  |     |     17px
//  |    41px    |
//  |     |    first ---> font size 13px
// 72px   |      |
//  |   second   |
//  |     |     42px
//  |    18px    |
//  v     v      v
// @ 100%
//  ^     ^      ^
//  |     |     12px
//  |    28px    |
//  |     |    first ---> font size 8px
// 48px   |      |
//  |   second   |
//  |     |     28px
//  |    12px    |
//  v     v      v
pub const FIRST_LINE_MIDPOINT_OFFSET_FROM_TOP: Unscaled<i32> = Unscaled::new(15);
pub const SECOND_LINE_MIDPOINT_OFFSET_FROM_TOP: Unscaled<i32> = Unscaled::new(31);
pub const LABEL_WIDTH: Unscaled<i32> = Unscaled::new(32);
pub const RIGHT_COLUMN_WIDTH: Unscaled<i32> = Unscaled::new(*LABEL_WIDTH.as_inner() + 28);
pub const WINDOW_WIDTH: Unscaled<i32> = Unscaled::new(170);
pub const DEFAULT_OFFSET_FROM_RIGHT: Unscaled<i32> = if cfg!(debug_assertions) {
    // Shift over when debugging so it's easy to compare with the installed version
    Unscaled::new(575)
} else {
    Unscaled::new(375)
};

// File names
pub const LOG_FILE_NAME: &str = "infoband.log";
pub const CONFIG_FILE_NAME: &str = "infoband.json";

// User messages
pub const UM_ENABLE_DEBUG_PAINT: WPARAM = WPARAM(1);
pub const UM_SET_OFFSET_FROM_RIGHT: WPARAM = WPARAM(2);
pub const UM_INITIAL_METRICS: WPARAM = WPARAM(3);
pub const UM_INITIAL_Z_ORDER: WPARAM = WPARAM(4);
pub const UM_INITIAL_PAINT: WPARAM = WPARAM(5);

// Timer ids
pub const IDT_FETCH_TIMER: WPARAM = WPARAM(1);
pub const IDT_REDRAW_TIMER: WPARAM = WPARAM(2);
pub const IDT_Z_ORDER_TIMER: WPARAM = WPARAM(3);

// Timer intervals
pub const FETCH_TIMER_MS: u32 = 1000;
pub const REDRAW_TIMER_MS: u32 = 5 * 1000;
pub const Z_ORDER_TIMER_MS: u32 = 50;

// Timer coalescing delays
pub const FETCH_TIMER_COALESCE: u32 = 1000;
pub const REDRAW_TIMER_COALESCE: u32 = 1000;
pub const Z_ORDER_TIMER_COALESCE: u32 = TIMERV_DEFAULT_COALESCING; // usually something short like 32ms

// Metrics
pub const SAMPLE_COUNT: usize = 8;
pub const EXPONENTIAL_DECAY_ALPHA: f64 = 0.631; // 0.631^5 = 0.1, so 90% of the weight is for the last 5 samples

// Shell hook messages
pub const HSHELL_WINDOWACTIVATED: WPARAM = WPARAM(0x4);
pub const HSHELL_RUDEAPPACTIVATED: WPARAM = WPARAM(0x8004);
