use crate::constants::{DEFAULT_MIC_HOTKEY, DEFAULT_OFFSET_FROM_RIGHT};
use crate::utils::Unscaled;
use argh::FromArgs;
use serde::{Deserialize, Serialize};

/// Computer info "deskband".
#[derive(FromArgs)]
pub struct Cli {
    /// logging verbosity (-v debug -v -v trace)
    #[argh(switch, short = 'v')]
    pub verbose: u8,

    /// whether to make the window more visible and interactible for debugging
    #[argh(switch)]
    pub debug_paint: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigFile {
    pub offset_from_right: Unscaled<i32>,
    pub mic_hotkey: Option<MicrophoneHotkey>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            offset_from_right: DEFAULT_OFFSET_FROM_RIGHT,
            mic_hotkey: DEFAULT_MIC_HOTKEY,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct MicrophoneHotkey {
    pub virtual_key_code: u16,
    #[serde(default)]
    pub win: bool,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}
