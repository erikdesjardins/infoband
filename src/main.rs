#![allow(non_snake_case)]
#![deny(unsafe_op_in_unsafe_fn)]

use log::LevelFilter;
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::Config;
use std::env;
use windows::core::Error;

mod defer;

mod ext;
mod metrics;
mod module;
mod opt;
mod proc;
mod window;

fn main() -> Result<(), Error> {
    let opt::Options {
        verbose,
        noninteractive,
        bordered,
    } = argh::from_env();

    log4rs::init_config(
        Config::builder()
            .appender(Appender::builder().build(
                "default",
                if noninteractive {
                    let mut path = env::current_exe().unwrap();
                    path.set_file_name("infoband.log");
                    Box::new(FileAppender::builder().build(path).unwrap())
                } else {
                    Box::new(ConsoleAppender::builder().target(Target::Stderr).build())
                },
            ))
            .build(Root::builder().appender("default").build(match verbose {
                0 => LevelFilter::Info,
                1 => LevelFilter::Debug,
                _ => LevelFilter::Trace,
            }))
            .unwrap(),
    )
    .unwrap();

    window::create_and_run_message_loop(bordered)?;

    Ok(())
}
