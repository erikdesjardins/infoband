use crate::module;
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::Config;
use windows::Win32::Foundation::HMODULE;

/// Initializes logging to a file next to the current DLL.
///
/// # Safety
///
/// `dll_module` must be a valid handle to the current DLL.
pub unsafe fn init(dll_module: HMODULE) {
    // SAFETY: dll_module validity requirement forwarded to caller
    let mut path = unsafe { module::get_dll_path(dll_module) };
    path.set_file_name("infoband.log");

    let default = FileAppender::builder().build(path).unwrap();
    let config = Config::builder()
        .appender(Appender::builder().build("default", Box::new(default)))
        .build(
            Root::builder()
                .appender("default")
                .build(LevelFilter::Trace),
        )
        .unwrap();

    log4rs::init_config(config).unwrap();
}
