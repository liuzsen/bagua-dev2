//! Postgres connection pool for Diesel

use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context, Result};
use diesel_async::pooled_connection::deadpool::{Object, Pool};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;
use serde::{Deserialize, Serialize};

/// pg pool connection
pub type PgConn = Object<AsyncPgConnection>;
pub type PgPool = Pool<AsyncPgConnection>;

static POOL: OnceLock<PgPool> = OnceLock::new();

/// Postgres connection pool config
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PgPoolConfig {
    pub min_conn: u32,
    pub max_conn: u32,
    pub url: String,
}

impl Default for PgPoolConfig {
    fn default() -> Self {
        Self {
            min_conn: 1,
            max_conn: 10,
            url: "postgresql://postgres:postgres@127.0.0.1:5432/postgres".to_string(),
        }
    }
}

/// Initialize a global postgres connection pool
pub async fn init(config: &PgPoolConfig) -> Result<()> {
    init_custom_pool(&POOL, config).await?;
    check_connections_loop(POOL.get().unwrap());

    Ok(())
}

fn check_connections_loop(pool: &'static PgPool) {
    tokio::spawn(async move {
        // Check every 30 seconds. Delete connections older than 1 minute
        let interval = Duration::from_secs(30);
        let max_age = Duration::from_secs(60);
        loop {
            tokio::time::sleep(interval).await;

            pool.retain(|_, metrics| metrics.last_used() < max_age);
        }
    });
}

/// Get a postgres connection
///
/// # Panics
///
/// Panics if the global postgres connection pool has not been initialized
pub async fn pg_conn() -> Result<PgConn> {
    let conn = POOL.get().unwrap().get().await?;
    Ok(conn)
}

/// Initialize a custom postgres connection pool
pub async fn init_custom_pool(
    pool: &'static OnceLock<PgPool>,
    config: &PgPoolConfig,
) -> Result<()> {
    if pool.get().is_some() {
        return Ok(());
    }

    let url_cfg = AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(&config.url);
    let built_pool = Pool::builder(url_cfg)
        .max_size(config.max_conn as usize)
        .build()?;
    let _conn = built_pool.get().await.with_context(|| {
        format!(
            "Failed to get a postgres connection after building the pool. URL = {}",
            config.url
        )
    })?;

    pool.get_or_init(|| built_pool);

    Ok(())
}

/// Get the global postgres connection pool
pub fn get_pool() -> &'static PgPool {
    POOL.get().unwrap()
}
