use crate::opt::MicrophoneHotkey;
use crate::utils::Unscaled;
use windows::Win32::Foundation::{COLORREF, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_OEM_2;
use windows::Win32::UI::WindowsAndMessaging::{self, TIMERV_DEFAULT_COALESCING};

// Startup parameters
pub const EXISTING_PROCESS_SHUTDOWN_MS: u32 = 1000;

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
// Microphone warning will be placed in the horizontal center of the display
pub const MICROPHONE_WARNING_WIDTH: Unscaled<i32> = Unscaled::new(78); // ~ 48 * 1.618 (golden ratio)

// Colors
pub const DEBUG_BACKGROUND_COLOR: COLORREF = COLORREF(0x00_77_77); // yellow
pub const MICROPHONE_WARNING_COLOR: COLORREF = COLORREF(0x00_00_99); // red

// File names
pub const LOG_FILE_NAME: &str = "infoband.log";
pub const CONFIG_FILE_NAME: &str = "infoband.json";
pub const PID_FILE_NAME: &str = "infoband.pid";

// Configuration
pub const DEFAULT_MIC_HOTKEY: Option<MicrophoneHotkey> = if cfg!(debug_assertions) {
    // Enable by default when debugging so it's easier to test
    Some(MicrophoneHotkey {
        // Slash / question mark
        virtual_key_code: VK_OEM_2.0,
        win: true,
        ctrl: false,
        shift: false,
        alt: false,
    })
} else {
    None
};
// Enable by default when debugging so it's easier to test
pub const DEFAULT_KEEP_AWAKE_WHILE_UNLOCKED: bool = cfg!(debug_assertions);

// User messages
pub const UM_ENABLE_KEEP_AWAKE: WPARAM = WPARAM(1);
pub const UM_ENABLE_DEBUG_PAINT: WPARAM = WPARAM(2);
pub const UM_INITIAL_METRICS: WPARAM = WPARAM(3);
pub const UM_INITIAL_MIC_STATE: WPARAM = WPARAM(4);
pub const UM_INITIAL_RENDER: WPARAM = WPARAM(5);
pub const UM_QUEUE_TRAY_POSITION_CHECK: WPARAM = WPARAM(6);
pub const UM_QUEUE_MIC_STATE_CHECK: WPARAM = WPARAM(7);

// Timer ids
pub const IDT_FETCH_AND_REDRAW_TIMER: WPARAM = WPARAM(1);
pub const IDT_TRAY_POSITION_TIMER: WPARAM = WPARAM(2);
pub const IDT_Z_ORDER_TIMER: WPARAM = WPARAM(3);
pub const IDT_MIC_STATE_TIMER: WPARAM = WPARAM(4);

// Timer intervals
pub const FETCH_TIMER_MS: u32 = 1000;
pub const REDRAW_EVERY_N_FETCHES: usize = 5;
pub const TRAY_POSITION_TIMER_MS: u32 = 10;
pub const Z_ORDER_TIMER_MS: u32 = 50;
pub const MIC_STATE_TIMER_MS: u32 = 10;

// Timer coalescing delays
pub const FETCH_AND_REDRAW_TIMER_COALESCE: u32 = 1000;
pub const TRAY_POSITION_TIMER_COALESCE: u32 = TIMERV_DEFAULT_COALESCING; // usually something short like 32ms
pub const Z_ORDER_TIMER_COALESCE: u32 = TIMERV_DEFAULT_COALESCING; // usually something short like 32ms
pub const MIC_STATE_TIMER_COALESCE: u32 = TIMERV_DEFAULT_COALESCING; // usually something short like 32ms

// Metrics
pub const SAMPLE_COUNT: usize = 8;
pub const EXPONENTIAL_DECAY_ALPHA: f64 = 0.631; // 0.631^5 = 0.1, so 90% of the weight is for the last 5 samples

// Shell hook messages
pub const HSHELL_WINDOWACTIVATED: WPARAM = WPARAM(0x4);
pub const HSHELL_RUDEAPPACTIVATED: WPARAM = WPARAM(0x8004);

// WTS session change messages
pub const WTS_SESSION_LOGON: WPARAM = WPARAM(WindowsAndMessaging::WTS_SESSION_LOGON as _);
pub const WTS_SESSION_LOGOFF: WPARAM = WPARAM(WindowsAndMessaging::WTS_SESSION_LOGOFF as _);
pub const WTS_SESSION_LOCK: WPARAM = WPARAM(WindowsAndMessaging::WTS_SESSION_LOCK as _);
pub const WTS_SESSION_UNLOCK: WPARAM = WPARAM(WindowsAndMessaging::WTS_SESSION_UNLOCK as _);

// Hotkey ids
pub const HOTKEY_MIC_MUTE: WPARAM = WPARAM(1);
