use qobuz_client::client::{ApiConfig, AudioQuality};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite, SqlitePool};
use std::{path::PathBuf, str::FromStr};

use crate::{
    acquire, get_one, query,
    state::{app::StateKey, Bytes},
};

#[derive(Debug, Clone)]
pub struct Database {
    pool: Pool<Sqlite>,
}

pub async fn new() -> Database {
    let database_url = if let Ok(url) = std::env::var("DATABASE_URL") {
        PathBuf::from(url)
    } else {
        let mut url = dirs::data_local_dir().unwrap();
        url.push("hifi-rs");
        url.set_file_name("data.db");

        url
    };

    let pool = SqlitePool::connect(database_url.to_str().unwrap())
        .await
        .expect("failed to open database");

    let db = Database { pool };
    db.create_config().await;

    db
}

impl Database {
    pub async fn insert<K, T>(&self, key: StateKey, value: T)
    where
        K: FromStr,
        T: Serialize,
    {
        if let Ok(serialized) = bincode::serialize(&value) {
            let mut conn = acquire!(self);
            let key = key.as_str();

            sqlx::query!(
                r#"
                INSERT INTO state (key,value)
                VALUES (?1, ?2)
                "#,
                key,
                serialized
            )
            .execute(&mut conn)
            .await
            .expect("database failure");
        }
    }

    pub async fn get<'a, K, T>(&self, key: StateKey) -> Option<T>
    where
        K: FromStr,
        T: Into<T> + From<Bytes> + Deserialize<'a>,
    {
        let mut conn = acquire!(self);
        let key = key.as_str();

        if let Ok(rec) = sqlx::query!(
            r#"
            SELECT value FROM state
            WHERE key=?1
            "#,
            key,
        )
        .fetch_one(&mut conn)
        .await
        {
            let bytes: Bytes = rec.value.into();
            Some(bytes.into())
        } else {
            None
        }
    }

    pub async fn set_username(&self, username: String) {
        let mut conn = acquire!(self);
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

    pub async fn set_password(&self, password: String) {
        let mut conn = acquire!(self);
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

    pub async fn set_user_token(&self, token: String) {
        let mut conn = acquire!(self);
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

    pub async fn set_app_id(&self, id: String) {
        let mut conn = acquire!(self);
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

    pub async fn set_active_secret(&self, secret: String) {
        let mut conn = acquire!(self);
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

    pub async fn set_default_quality(&self, quality: AudioQuality) {
        let mut conn = acquire!(self);

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

    pub async fn create_config(&self) {
        let mut conn = acquire!(self);
        let rowid = 1;
        query!(
            r#"
            INSERT OR IGNORE INTO config (ROWID) VALUES (?1);
            "#,
            conn,
            rowid
        );
    }

    pub async fn get_config(&self) -> ApiConfig {
        let mut conn = acquire!(self);
        get_one!(
            r#"
            SELECT * FROM config
            WHERE ROWID = 1;
            "#,
            ApiConfig,
            conn
        )
    }
}
