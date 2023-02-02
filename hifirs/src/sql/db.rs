use qobuz_client::client::{ApiConfig, AudioQuality};
use sqlx::{sqlite::SqliteConnectOptions, Pool, Sqlite, SqlitePool};
use std::path::PathBuf;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::{acquire, get_one, query};

#[derive(Debug, Clone)]
pub struct Database {
    pool: Pool<Sqlite>,
    quit_sender: Sender<bool>,
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

    let (quit_sender, _) = tokio::sync::broadcast::channel::<bool>(1);

    let db = Database { pool, quit_sender };
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

    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub fn quitter(&self) -> Receiver<bool> {
        self.quit_sender.subscribe()
    }

    pub fn quit(&self) {
        self.quit_sender
            .send(true)
            .expect("failed to send quit message");

        futures::executor::block_on(async {
            self.pool.close().await;
        });
    }
}
