use argh::FromArgs;

/// Computer info "deskband".
#[derive(FromArgs)]
pub struct Options {
    /// logging verbosity (-v debug -v -v trace)
    #[argh(switch, short = 'v')]
    pub verbose: u8,

    /// whether to run in non-interactive mode
    ///
    /// For example, when running as a Windows service.
    /// Primarily, redirects logging to a file instead of stderr.
    #[argh(switch)]
    pub noninteractive: bool,

    /// whether to make the window more visible and interactible for debugging
    #[argh(switch)]
    pub debug_paint: bool,
}
