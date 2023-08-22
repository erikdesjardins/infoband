#![allow(
    non_snake_case,
    non_camel_case_types,
    unstable_name_collisions,
    clippy::let_unit_value,
    clippy::if_same_then_else
)]
#![deny(unsafe_op_in_unsafe_fn)]

use log::LevelFilter;
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;
use std::env;
use windows::core::Error;

mod macros;

mod constants;
mod metrics;
mod module;
mod opt;
mod stats;
mod utils;
mod window;

fn main() -> Result<(), Error> {
    let opt::Options {
        verbose,
        noninteractive,
        debug_paint,
    } = argh::from_env();

    log4rs::init_config(
        Config::builder()
            .appender(Appender::builder().build("default", {
                let encoder = Box::new(PatternEncoder::new("[{date(%Y-%m-%d %H:%M:%S%.3f)} {highlight({level}):5} {target}] {highlight({message})}{n}"));
                if noninteractive {
                    let mut path = env::current_exe().unwrap();
                    path.set_file_name("infoband.log");
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

    window::make_process_dpi_aware()?;

    window::create_and_run_message_loop(debug_paint)?;

    Ok(())
}
