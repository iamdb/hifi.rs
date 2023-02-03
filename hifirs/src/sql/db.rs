use qobuz_client::client::{ApiConfig, AudioQuality};
use sqlx::{sqlite::SqliteConnectOptions, Pool, Sqlite, SqlitePool};
use std::path::PathBuf;

use crate::{
    acquire, get_one, query,
    state::{app::PlayerState, TrackListType},
};

#[derive(Debug, Clone)]
pub struct Database {
    pool: Pool<Sqlite>,
}

pub async fn new() -> Database {
    let database_url = if let Ok(url) = std::env::var("DATABASE_URL") {
        PathBuf::from(url.replace("sqlite://", ""))
    } else {
        let mut url = dirs::data_local_dir().unwrap();
        url.push("hifi-rs");

        if !url.exists() {
            std::fs::create_dir_all(url.clone()).expect("failed to create database directory");
        }

        url.push("data.db");

        url
    };

    debug!("DATABASE_URL: {}", database_url.to_string_lossy());

    let options = SqliteConnectOptions::new()
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .filename(database_url)
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(options)
        .await
        .expect("failed to open database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migration failed");

    let db = Database { pool };
    db.create_config().await;

    db
}

impl Database {
    pub async fn clear_state(&self) {
        if let Ok(mut conn) = acquire!(self) {
            sqlx::query("DELETE FROM state WHERE state.key != 'active_screen'")
                .execute(&mut conn)
                .await
                .expect("failed to clear state");
        }
    }

    pub async fn set_username(&self, username: String) {
        if let Ok(mut conn) = acquire!(self) {
            query!(
                r#"
            UPDATE config
            SET username=?1
            WHERE ROWID = 1
            "#,
                conn,
                username
            );
        }
    }

    pub async fn set_password(&self, password: String) {
        if let Ok(mut conn) = acquire!(self) {
            query!(
                r#"
            UPDATE config
            SET password=?1
            WHERE ROWID = 1
            "#,
                conn,
                password
            );
        }
    }

    pub async fn set_user_token(&self, token: String) {
        if let Ok(mut conn) = acquire!(self) {
            query!(
                r#"
            UPDATE config
            SET user_token=?1
            WHERE ROWID = 1
            "#,
                conn,
                token
            );
        }
    }

    pub async fn set_app_id(&self, id: String) {
        if let Ok(mut conn) = acquire!(self) {
            query!(
                r#"
            UPDATE config
            SET app_id=?1
            WHERE ROWID = 1
            "#,
                conn,
                id
            );
        }
    }

    pub async fn set_active_secret(&self, secret: String) {
        if let Ok(mut conn) = acquire!(self) {
            query!(
                r#"
            UPDATE config
            SET active_secret=?1
            WHERE ROWID = 1
            "#,
                conn,
                secret
            );
        }
    }

    pub async fn set_default_quality(&self, quality: AudioQuality) {
        if let Ok(mut conn) = acquire!(self) {
            let quality_id = quality as i32;

            query!(
                r#"
            UPDATE config
            SET default_quality=?1
            WHERE ROWID = 1
            "#,
                conn,
                quality_id
            );
        }
    }

    pub async fn create_config(&self) {
        if let Ok(mut conn) = acquire!(self) {
            let rowid = 1;
            query!(
                r#"
            INSERT OR IGNORE INTO config (ROWID) VALUES (?1);
            "#,
                conn,
                rowid
            );
        }
    }

    pub async fn get_config(&self) -> Option<ApiConfig> {
        if let Ok(mut conn) = acquire!(self) {
            Some(get_one!(
                r#"
            SELECT * FROM config
            WHERE ROWID = 1;
            "#,
                ApiConfig,
                conn
            ))
        } else {
            None
        }
    }

    pub async fn persist_state(&self, state: PlayerState) {
        if let Ok(mut conn) = acquire!(self) {
            if let (Some(current_track), Some(playback_track_index)) =
                (state.current_track(), state.current_track_index())
            {
                let playback_track_index = playback_track_index as i32;
                let playback_track_id = current_track.track.id;
                let playback_position = state.position().inner_clocktime().mseconds() as i32;
                let playback_entity_type = state.list_type();
                let playback_entity_id = match playback_entity_type {
                    TrackListType::Album => {
                        state.album().expect("failed to get album id").id.clone()
                    }
                    TrackListType::Playlist => state
                        .playlist()
                        .expect("failed to get playlist id")
                        .id
                        .to_string(),
                    TrackListType::Track => "".to_string(),
                    TrackListType::Unknown => "".to_string(),
                };

                if !playback_entity_id.is_empty() {
                    let playback_entity_type = playback_entity_type.to_string();

                    sqlx::query!(
                        r#"INSERT INTO player_state VALUES(NULL,?1,?2,?3,?4,?5);"#,
                        playback_track_id,
                        playback_position,
                        playback_track_index,
                        playback_entity_id,
                        playback_entity_type
                    )
                    .execute(&mut conn)
                    .await
                    .expect("database failure");
                }
            }
        }
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}
