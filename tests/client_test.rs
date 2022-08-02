use hifi_rs::{self, qobuz::client};
use tokio_test::assert_ok;

mod common;

/// NOTE: Tests must be run consecutively. `cargo test -- --test-threads=1`

#[tokio::test]
async fn can_create_client() {
    let (app_state, creds) = common::setup(1);

    assert_ok!(client::new(app_state.clone(), creds).await);

    common::teardown(1);
}

#[tokio::test]
async fn can_use_methods() {
    let (app_state, creds) = common::setup(2);

    let mut client = client::new(app_state, creds)
        .await
        .expect("failed to create client");

    assert_ok!(client.user_playlists().await);
    assert_ok!(
        client
            .search_albums("pink_floyd".to_string(), Some(10))
            .await
    );
    assert_ok!(client.search_artists("pink_floyd".to_string()).await);

    common::teardown(2);
}
