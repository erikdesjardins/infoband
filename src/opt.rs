use crate::constants::DEFAULT_OFFSET_FROM_RIGHT;
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
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            offset_from_right: DEFAULT_OFFSET_FROM_RIGHT,
        }
    }
}
