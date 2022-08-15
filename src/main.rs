#[tokio::main]
async fn main() -> Result<(), hifi_rs::cli::Error> {
    hifi_rs::cli::run().await
}
