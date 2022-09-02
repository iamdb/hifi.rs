extern crate pretty_env_logger;
#[macro_use]
extern crate log;

#[macro_use]
pub mod cli;
mod mpris;
#[macro_use]
mod player;
mod qobuz;
#[macro_use]
pub mod state;
#[macro_use]
pub mod ui;
pub mod util;

const REFRESH_RESOLUTION: u64 = 500;
pub const TEST_TEMP_PATH: &str = "/tmp/hifirs_test";
