use crate::sql::db::Database;
use hifirs_qobuz_api::{
    client::{
        api::{self, Client},
        AudioQuality,
    },
    Credentials,
};
use serde::{Deserialize, Serialize};

pub type Result<T, E = hifirs_qobuz_api::Error> = std::result::Result<T, E>;

pub mod album;
pub mod artist;
pub mod playlist;
pub mod track;

pub async fn make_client(
    username: Option<String>,
    password: Option<String>,
    db: &Database,
) -> Result<Client> {
    let mut client = api::new(None, None, None, None).await?;
    if username.is_some() || password.is_some() {
        client.set_credentials(Credentials { username, password });
    }

    setup_client(&mut client, db).await
}

/// Setup app_id, secret and user credentials for authentication
pub async fn setup_client(client: &mut Client, db: &Database) -> Result<Client> {
    info!("setting up the api client");

    if let Some(config) = db.get_config().await {
        let mut refresh_config = false;

        if let Some(quality) = config.default_quality {
            info!("using default quality from cache: {}", quality);
            let quality: AudioQuality = quality.into();
            client.set_default_quality(quality);
        }

        if let Some(app_id) = config.app_id {
            debug!("using app_id from cache");
            client.set_app_id(app_id);
        } else {
            debug!("app_id not found, will have to refresh config");
            refresh_config = true;
        }

        if let Some(secret) = config.active_secret {
            debug!("using active secret from cache");
            client.set_active_secret(secret);
        } else {
            debug!("active_secret not found, will have to refresh config");
            refresh_config = true;
        }

        if let Some(token) = config.user_token {
            info!("using token from cache");
            client.set_token(token);

            if refresh_config {
                client.refresh().await?;
                client.test_secrets().await?;

                if let Some(id) = client.get_app_id() {
                    db.set_app_id(id).await;
                }

                if let Some(secret) = client.get_active_secret() {
                    db.set_active_secret(secret).await;
                }
            }
        } else if let (Some(username), Some(password)) = (config.username, config.password) {
            info!("setting auth using username and password from cache");
            client.set_credentials(Credentials {
                username: Some(username),
                password: Some(password),
            });

            if refresh_config {
                client.refresh().await?;

                if let Some(id) = client.get_app_id() {
                    db.set_app_id(id).await;
                }
            }

            client.login().await?;
            client.test_secrets().await?;

            if let Some(token) = client.get_token() {
                db.set_user_token(token).await;
            }

            if let Some(secret) = client.get_active_secret() {
                db.set_active_secret(secret).await;
            }
        } else {
            return Err(hifirs_qobuz_api::Error::NoCredentials);
        }
    }

    Ok(client.clone())
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Composer {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub albums_count: i64,
    pub image: Option<Image>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    pub small: String,
    pub thumbnail: Option<String>,
    pub large: String,
    pub back: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackURL {
    pub track_id: i32,
    pub duration: i32,
    pub url: String,
    pub format_id: i32,
    pub mime_type: String,
    pub sampling_rate: f64,
    pub bit_depth: i32,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub login: String,
}
