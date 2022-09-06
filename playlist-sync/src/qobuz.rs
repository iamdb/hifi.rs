use qobuz_client::client::api::{Client, Credentials as QobuzCredentials};

pub struct Qobuz {
    client: Client,
}

pub async fn new() -> Qobuz {
    let creds = QobuzCredentials {
        username: Some(env!("QOBUZ_USERNAME").to_string()),
        password: Some(env!("QOBUZ_PASSWORD").to_string()),
    };

    let mut client = qobuz_client::client::api::new(Some(creds.clone()), None, None, None, None)
        .await
        .expect("failed to create client");

    client.refresh().await;
    client.login().await.expect("failed to login");

    Qobuz { client }
}
