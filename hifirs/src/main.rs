use log::debug;

#[tokio::main]
async fn main() -> Result<(), hifi_rs::cli::Error> {
    debug!("test");
    hifi_rs::cli::run().await
}
