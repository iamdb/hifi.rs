use hifi_rs::state::AudioQuality;
use tokio_test::assert_ok;

mod common;

#[tokio::test]
async fn can_use_methods() {
    let (_, client, path) = common::setup().await;

    assert_ok!(client.user_playlists().await);
    let album_response = assert_ok!(
        client
            .search_albums("a love supreme".to_string(), Some(10))
            .await
    );
    assert_eq!(album_response.albums.items.len(), 10);
    assert_ok!(client.album("lhrak0dpdxcbc".to_string()).await);
    let artist_response = assert_ok!(
        client
            .search_artists("pink floyd".to_string(), Some(10))
            .await
    );
    assert_eq!(artist_response.artists.items.len(), 10);
    assert_ok!(client.artist(148745, Some(10)).await);
    assert_ok!(client.track(155999429).await);
    assert_ok!(
        client
            .track_url(155999429, Some(AudioQuality::Mp3), None)
            .await
    );

    common::teardown(path);
}
