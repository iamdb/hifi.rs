//extern crate pretty_env_logger;
#[macro_use]
extern crate tracing;

#[macro_use]
pub mod cli;
#[cfg(target_os = "linux")]
mod mpris;
#[macro_use]
mod player;
mod qobuz;
#[macro_use]
pub mod state;
#[macro_use]
pub mod ui;
pub mod cursive;
#[macro_use]
pub mod sql;

const REFRESH_RESOLUTION: u64 = 1000;
pub const TEST_TEMP_PATH: &str = "/tmp/hifirs_test";
