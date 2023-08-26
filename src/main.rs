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
use std::fs::{self, File};
use std::{env, io};
use windows::core::Error;
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

mod macros;

mod constants;
mod metrics;
mod opt;
mod stats;
mod utils;
mod window;

fn main() -> Result<(), Error> {
    let opt::Cli {
        verbose,
        debug_paint,
    } = argh::from_env();

    // Init logging as early as possible.
    init_logging(verbose);

    let opt::ConfigFile { offset_from_right } = load_config_file();

    let instance = get_module_handle()?;
    window::make_process_dpi_aware()?;
    window::create_and_run_message_loop(instance, offset_from_right, debug_paint)?;

    Ok(())
}

fn init_logging(verbose: u8) {
    log4rs::init_config(
        Config::builder()
            .appender(Appender::builder().build("default", {
                let encoder = Box::new(PatternEncoder::new("[{date(%Y-%m-%d %H:%M:%S%.3f)} {highlight({level}):5} {target}] {highlight({message})}{n}"));
                if !cfg!(debug_assertions) {
                    let mut path = env::current_exe().unwrap();
                    path.set_file_name(LOG_FILE_NAME);
                    Box::new(FileAppender::builder().build(path).unwrap())
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

fn load_config_file() -> opt::ConfigFile {
    let default_config = opt::ConfigFile::default();

    let mut path = env::current_exe().unwrap();
    path.set_file_name(CONFIG_FILE_NAME);

    match File::open(&path) {
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
            match File::create(&path) {
                Ok(file) => match serde_json::to_writer_pretty(file, &default_config) {
                    Ok(()) => {}
                    Err(e) => {
                        log::warn!("Failed to write config file `{}`: {}", path.display(), e);
                        if let Err(e) = fs::remove_file(&path) {
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

fn get_module_handle() -> Result<HINSTANCE, Error> {
    // SAFETY: no safety requirements when passing null
    let module = unsafe { GetModuleHandleW(None)? };
    Ok(HINSTANCE::from(module))
}
