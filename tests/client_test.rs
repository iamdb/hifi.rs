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
async fn can_login() {
    let (app_state, creds) = common::setup(2);

    let mut client = client::new(app_state.clone(), creds)
        .await
        .expect("failed to create client");
    assert_ok!(client.login().await);

    common::teardown(2);
}

#[tokio::test]
async fn can_fetch_user_playlists() {
    let (app_state, creds) = common::setup(3);

    let mut client = client::new(app_state, creds)
        .await
        .expect("failed to create client");

    assert_ok!(client.user_playlists().await);

    common::teardown(3);
}

// #[tokio::test]
// async fn can_search_albums() {
//     let (app_state, creds) = common::setup(3);
//
//     let mut client = client::new(app_state, creds)
//         .await
//         .expect("failed to create client");
//
//     let results = assert_ok!(client.search_albums("pink floyd".to_string(), 100).await);
//
//     assert!(results.albums.items.len() == 100);
//
//     common::teardown(3);
// }
