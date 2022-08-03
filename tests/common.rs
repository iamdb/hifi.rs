use nanoid::nanoid;
use std::{path::PathBuf, str::FromStr};

use hifi_rs::{
    qobuz::client::{self, Client},
    state::{self, app::AppState},
    Credentials,
};

const TEMP_PATH: &str = "/tmp/hifirs_test";

pub async fn setup() -> (AppState, Client, PathBuf) {
    let id = nanoid!();
    let path_string = format!("{}_{}", TEMP_PATH, id);
    let path = PathBuf::from_str(path_string.as_str()).expect("failed to create path");
    let app_state = state::app::new(path.clone()).expect("failed to create database");
    let creds = Credentials {
        username: Some(env!("QOBUZ_USERNAME").to_string()),
        password: Some(env!("QOBUZ_PASSWORD").to_string()),
    };

    let client = client::new(app_state.clone(), creds.clone())
        .await
        .expect("failed to create client");

    (app_state, client, path)
}

pub fn teardown(path: PathBuf) {
    std::fs::remove_dir_all(path).expect("failed to remove temp directory");
}
