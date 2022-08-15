extern crate pretty_env_logger;
#[macro_use]
extern crate log;

pub mod cli;
mod mpris;
#[macro_use]
mod player;
mod qobuz;
#[macro_use]
mod state;
#[macro_use]
mod ui;
mod util;

pub const REFRESH_RESOLUTION: u64 = 500;
pub const TEST_TEMP_PATH: &str = "/tmp/hifirs_test";
