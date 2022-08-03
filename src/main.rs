use clap::Parser;
use hifi_rs::{cli::Cli, state, Credentials};

#[tokio::main]
async fn main() -> Result<(), String> {
    // PARSE CLI ARGS
    let cli = Cli::parse();

    // DATABASE DIRECTORY
    let mut base_dir = dirs::data_local_dir().unwrap();
    base_dir.push("hifi-rs");

    // SETUP DATABASE
    let app_state = state::app::new(base_dir);

    hifi_rs::cli(
        cli.command,
        app_state,
        Credentials {
            username: cli.username,
            password: cli.password,
        },
    )
    .await
}
