#![allow(
    non_snake_case,
    non_camel_case_types,
    unstable_name_collisions,
    clippy::let_unit_value,
    clippy::if_same_then_else
)]
#![deny(unsafe_op_in_unsafe_fn)]
// Prevent the automatic console window you get on startup.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::constants::{CONFIG_FILE_NAME, LOG_FILE_NAME};
use log::LevelFilter;
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;
use std::env;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use windows::core::Result;
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

mod macros;

mod constants;
mod metrics;
mod opt;
mod perf;
mod stats;
mod utils;
mod window;

fn main() -> Result<()> {
    let opt::Cli {
        verbose,
        debug_paint,
    } = argh::from_env();

    // Init logging as early as possible.
    let config = if cfg!(debug_assertions) {
        // In debug builds, don't create log/config files
        init_logging(None, verbose);
        Default::default()
    } else {
        // In release (installed) builds, create log/config files in local appdata.
        let path = make_local_appdata_folder();
        init_logging(Some(&path.join(LOG_FILE_NAME)), verbose);
        load_config_file(&path.join(CONFIG_FILE_NAME))
    };

    let opt::ConfigFile { offset_from_right } = config;

    log::info!("Started up infoband {}", env!("CARGO_PKG_VERSION"));

    let instance = get_module_handle();

    if let Err(e) = window::make_process_dpi_aware() {
        log::error!("Failed to make process DPI aware: {}", e);
        return Err(e);
    }

    if let Err(e) = window::create_and_run_message_loop(instance, offset_from_right, debug_paint) {
        log::error!("Failed to create and run message loop: {}", e);
        return Err(e);
    }

    Ok(())
}

fn make_local_appdata_folder() -> PathBuf {
    let Some(local_appdata) = env::var_os("LOCALAPPDATA") else {
        panic!("Failed to get LOCALAPPDATA environment variable.");
    };

    let mut path = PathBuf::from(local_appdata);
    path.push("infoband");

    if let Err(e) = fs::create_dir_all(&path) {
        panic!(
            "Failed to create local appdata folder `{}`: {}",
            path.display(),
            e
        );
    }

    path
}

fn init_logging(path: Option<&Path>, verbose: u8) {
    log4rs::init_config(
        Config::builder()
            .appender(Appender::builder().build("default", {
                let encoder = Box::new(PatternEncoder::new("[{date(%Y-%m-%d %H:%M:%S%.3f)} {highlight({level}):5} {target}] {highlight({message})}{n}"));
                if let Some(path) = path {
                    Box::new(FileAppender::builder().encoder(encoder).build(path).unwrap())
                } else {
                    Box::new(
                        ConsoleAppender::builder()
                            .encoder(encoder)
                            .target(Target::Stderr)
                            .build(),
                    )
                }
            }))
            .build(Root::builder().appender("default").build(match verbose {
                0 => LevelFilter::Info,
                1 => LevelFilter::Debug,
                _ => LevelFilter::Trace,
            }))
            .unwrap(),
    )
    .unwrap();
}

fn load_config_file(path: &Path) -> opt::ConfigFile {
    let default_config = opt::ConfigFile::default();

    match File::open(path) {
        Ok(file) => match serde_json::from_reader(file) {
            Ok(config) => {
                log::info!("Loaded config from file `{}`", path.display());
                config
            }
            Err(e) => {
                log::error!("Failed to parse config file `{}`: {}", path.display(), e);
                default_config
            }
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            log::info!("Config file `{}` not found, creating", path.display());
            match File::create(path) {
                Ok(file) => match serde_json::to_writer_pretty(file, &default_config) {
                    Ok(()) => {}
                    Err(e) => {
                        log::warn!("Failed to write config file `{}`: {}", path.display(), e);
                        if let Err(e) = fs::remove_file(path) {
                            log::warn!("...and failed to delete the empty file: {}", e);
                        }
                    }
                },
                Err(e) => log::warn!("Failed to create config file `{}`: {}", path.display(), e),
            }
            default_config
        }
        Err(e) => {
            log::error!("Failed to load config file: {}", e);
            default_config
        }
    }
}

fn get_module_handle() -> HINSTANCE {
    // SAFETY: no safety requirements when passing null
    let module = unsafe { GetModuleHandleW(None).unwrap() };
    HINSTANCE::from(module)
}
