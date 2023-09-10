use crate::service::{Album, Artist};
use hifirs_qobuz_api::client::artist::Artist as QobuzArtist;

impl From<QobuzArtist> for Artist {
    fn from(a: QobuzArtist) -> Self {
        Self {
            id: a.id as usize,
            name: a.name,
            albums: a.albums.map(|a| {
                a.items
                    .into_iter()
                    .map(|a| a.into())
                    .collect::<Vec<Album>>()
            }),
        }
    }
}
