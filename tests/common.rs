use std::{path::PathBuf, str::FromStr};

use hifi_rs::{
    state::{self, app::AppState},
    Credentials,
};

const TEMP_PATH: &str = "/tmp/hifirs_test";

pub fn setup(num: i32) -> (AppState, Credentials) {
    let path = format!("{}_{}", TEMP_PATH, num);

    (
        state::app::new(PathBuf::from_str(path.as_str()).expect("failed to get path")),
        Credentials {
            username: Some(env!("QOBUZ_USERNAME").to_string()),
            password: Some(env!("QOBUZ_PASSWORD").to_string()),
        },
    )
}

pub fn teardown(num: i32) {
    let path = format!("{}_{}", TEMP_PATH, num);

    std::fs::remove_dir_all(PathBuf::from_str(path.as_str()).expect("failed to get path"))
        .expect("failed to remove temp directory");
}
