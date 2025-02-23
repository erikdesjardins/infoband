#![allow(
    non_snake_case,
    non_camel_case_types,
    unstable_name_collisions,
    clippy::collapsible_else_if,
    clippy::if_same_then_else,
    clippy::let_unit_value,
    clippy::manual_non_exhaustive
)]
#![deny(unsafe_op_in_unsafe_fn)]
// Prevent the automatic console window you get on startup.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::constants::{CONFIG_FILE_NAME, LOG_FILE_NAME, PID_FILE_NAME};
use constants::EXISTING_PROCESS_SHUTDOWN_MS;
use log::LevelFilter;
use log4rs::Config;
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use std::env;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    PROCESS_TERMINATE, TerminateProcess, WaitForSingleObject,
};
use windows::core::{Error, Result, w};

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
        kill_and_write_pid_file(&path.join(PID_FILE_NAME));
        load_config_file(&path.join(CONFIG_FILE_NAME))
    };

    let opt::ConfigFile {
        offset_from_right,
        mic_hotkey,
    } = config;

    log::info!("Started up infoband {}", env!("CARGO_PKG_VERSION"));

    if let Err(e) = window::create_and_run_message_loop(offset_from_right, mic_hotkey, debug_paint)
    {
        log::error!("Failed to create and run message loop: {e}");
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
            "Failed to create local appdata folder `{}`: {e}",
            path.display()
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

fn kill_and_write_pid_file(path: &Path) {
    fn kill_existing_process(path: &Path) {
        let pid = match fs::read_to_string(path) {
            Ok(pid) => pid,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return,
            Err(e) => return log::warn!("Failed to read pid file `{}`: {e}", path.display()),
        };

        let pid = match pid.parse::<u32>() {
            Ok(pid) => pid,
            Err(e) => return log::warn!("Failed to parse pid file `{}`: {e}", path.display()),
        };

        let process = match unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
                false,
                pid,
            )
        } {
            Ok(process) => process,
            // This happens normally when the process has already exited.
            Err(e) => return log::debug!("Failed to open process {pid}: {e}"),
        };
        defer! {
            if let Err(e) = unsafe { CloseHandle(process) } {
                log::warn!("Failed to close process {pid}: {e}");
            }
        }

        let mut name = [0; 4096];
        let len = match unsafe { GetModuleFileNameExW(Some(process), None, &mut name) } {
            0 => {
                return log::warn!(
                    "Failed to get process name for pid {pid}: {}",
                    Error::from_win32()
                );
            }
            len => len,
        };
        let name = &name[..len as usize];

        let infoband = w!("infoband.exe");
        if !name.ends_with(unsafe { infoband.as_wide() }) {
            return log::debug!(
                "Not killing process {pid} (`{}`)",
                String::from_utf16_lossy(name)
            );
        }

        match unsafe { TerminateProcess(process, 0) } {
            Ok(()) => log::info!("Started termination of existing instance with pid {pid}"),
            Err(e) => return log::warn!("Failed to terminate process {pid}: {e}"),
        }

        match unsafe { WaitForSingleObject(process, EXISTING_PROCESS_SHUTDOWN_MS) } {
            WAIT_OBJECT_0 => {
                log::info!("Completed termination of existing instance with pid {pid}")
            }
            WAIT_TIMEOUT => log::warn!(
                "Existing instance with pid {pid} did not exit after {EXISTING_PROCESS_SHUTDOWN_MS}ms"
            ),
            _ => log::warn!(
                "Failed to wait for existing instance with pid {pid} to exit: {}",
                Error::from_win32()
            ),
        }
    }

    kill_existing_process(path);

    // SAFETY: not unsafe
    let current_pid = unsafe { GetCurrentProcessId() };

    match fs::write(path, current_pid.to_string()) {
        Ok(()) => log::debug!("Wrote pid {current_pid} to file `{}`", path.display()),
        Err(e) => log::warn!("Failed to write pid file `{}`: {e}", path.display()),
    }
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
                log::error!("Failed to parse config file `{}`: {e}", path.display());
                default_config
            }
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            log::info!("Config file `{}` not found, creating", path.display());
            match File::create(path) {
                Ok(file) => {
                    if let Err(e) = serde_json::to_writer_pretty(file, &default_config) {
                        log::warn!("Failed to write config file `{}`: {e}", path.display());
                        if let Err(e) = fs::remove_file(path) {
                            log::warn!("...and failed to delete the empty file: {e}");
                        }
                    }
                }
                Err(e) => log::warn!("Failed to create config file `{}`: {e}", path.display()),
            }
            default_config
        }
        Err(e) => {
            log::error!("Failed to load config file: {e}");
            default_config
        }
    }
}
