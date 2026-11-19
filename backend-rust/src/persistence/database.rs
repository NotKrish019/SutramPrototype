//! PostgreSQL Database Pool and Migration Management.
//!
//! Provides connection pooling, connection validation, graceful shutdown,
//! and version-controlled migration execution via SQLx.

use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;
use tracing::info;

use crate::config::AppConfig;
use crate::error::AppError;

/// Database manager holding the shared asynchronous connection pool.
#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Attempts to establish a connection pool to PostgreSQL using configuration.
    pub async fn connect(config: &AppConfig) -> Result<Self, AppError> {
        let url = config
            .database_url
            .as_deref()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("DATABASE_URL is not configured")))?;

        let pool = PgPoolOptions::new()
            .max_connections(config.database_max_connections)
            .min_connections(config.database_min_connections)
            .acquire_timeout(Duration::from_secs(config.database_connect_timeout_seconds))
            .connect(url)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "Failed to connect to PostgreSQL database: {}",
                    e
                ))
            })?;

        info!("Successfully established PostgreSQL connection pool");
        Ok(Self { pool })
    }

    /// Wraps an existing `PgPool` instance (e.g. for testing).
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns a reference to the underlying SQLx `PgPool`.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Executes all pending migrations from `backend-rust/migrations/`.
    pub async fn run_migrations(&self) -> Result<(), AppError> {
        info!("Running pending PostgreSQL database migrations...");
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "Failed to execute database migrations: {}",
                    e
                ))
            })?;
        info!("PostgreSQL migrations applied successfully");
        Ok(())
    }

    /// Health check ping to verify that the database is active and responsive.
    pub async fn ping(&self) -> Result<(), AppError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Database health check failed: {}", e))
            })?;
        Ok(())
    }

    /// Gracefully closes all connections in the pool.
    pub async fn close(&self) {
        info!("Closing PostgreSQL database connection pool...");
        self.pool.close().await;
    }
}
