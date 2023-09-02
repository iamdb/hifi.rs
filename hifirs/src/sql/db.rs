use hifirs_qobuz_api::client::{ApiConfig, AudioQuality};
use once_cell::sync::OnceCell;
use sqlx::{sqlite::SqliteConnectOptions, Pool, Sqlite, SqlitePool};
use std::path::PathBuf;

use crate::{
    acquire, get_one, query,
    state::app::{PlayerState, SavedState},
};

static POOL: OnceCell<Pool<Sqlite>> = OnceCell::new();

pub async fn init() {
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

    POOL.set(pool).expect("error setting static pool");
}

pub async fn clear_state() {
    if let Ok(mut conn) = acquire!() {
        sqlx::query("DELETE FROM state WHERE state.key != 'active_screen'")
            .execute(&mut *conn)
            .await
            .expect("failed to clear state");
    }
}

pub async fn set_username(username: String) {
    if let Ok(mut conn) = acquire!() {
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

pub async fn set_password(password: String) {
    if let Ok(mut conn) = acquire!() {
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

pub async fn set_user_token(token: String) {
    if let Ok(mut conn) = acquire!() {
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

pub async fn set_app_id(id: String) {
    if let Ok(mut conn) = acquire!() {
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

pub async fn set_active_secret(secret: String) {
    if let Ok(mut conn) = acquire!() {
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

pub async fn set_default_quality(quality: AudioQuality) {
    if let Ok(mut conn) = acquire!() {
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

pub async fn create_config() {
    if let Ok(mut conn) = acquire!() {
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

pub async fn get_config() -> Option<ApiConfig> {
    if let Ok(mut conn) = acquire!() {
        if let Ok(conf) = get_one!(
            r#"
            SELECT * FROM config
            WHERE ROWID = 1;
            "#,
            ApiConfig,
            conn
        ) {
            Some(conf)
        } else {
            None
        }
    } else {
        None
    }
}

pub async fn persist_state(state: PlayerState) {
    if let Ok(mut conn) = acquire!() {
        let saved_state: SavedState = state.into();
        let playback_entity_type = saved_state.playback_entity_type.to_string();

        sqlx::query!(
            r#"INSERT INTO player_state VALUES(NULL,?1,?2,?3,?4,?5);"#,
            saved_state.playback_track_id,
            saved_state.playback_position,
            saved_state.playback_track_index,
            saved_state.playback_entity_id,
            playback_entity_type
        )
        .execute(&mut *conn)
        .await
        .expect("database failure");
    }
}

pub async fn get_last_state() -> Option<SavedState> {
    if let Ok(mut conn) = acquire!() {
        if let Ok(state) = get_one!(
            r#"SELECT * FROM player_state ORDER BY rowid DESC LIMIT 1;"#,
            SavedState,
            conn
        ) {
            Some(state)
        } else {
            None
        }
    } else {
        None
    }
}

pub async fn close() {
    POOL.get().unwrap().close().await;
}
