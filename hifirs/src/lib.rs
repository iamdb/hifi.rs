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
pub mod cursive;
pub mod service;
#[macro_use]
pub mod sql;
pub mod websocket;

const REFRESH_RESOLUTION: u64 = 250;
pub const TEST_TEMP_PATH: &str = "/tmp/hifirs_test";
