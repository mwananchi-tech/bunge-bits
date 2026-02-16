use std::sync::LazyLock;

use anyhow::Context;
use sqlx::{migrate::Migrator, postgres::PgPoolOptions, PgPool};

use crate::{datastore::DataStore, domain::TIME_AGO_REGEX};

static MIGRATOR: Migrator = sqlx::migrate!();

#[derive(Debug, Clone)]
pub struct PgDataStore {
    pub pool: PgPool,
}

impl PgDataStore {
    /// Establish connection to database and create the streams table
    /// if not exists
    pub async fn init(database_url: &str) -> anyhow::Result<Self> {
        LazyLock::force(&TIME_AGO_REGEX);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .inspect_err(
                |e| tracing::error!(error = ?e, "Failed to establish connection to database"),
            )
            .context("Failed to connect to postgres database")?;

        MIGRATOR
            .run(&pool)
            .await
            .inspect_err(|e| tracing::error!(error = ?e, "Failed to run database migrations"))
            .context("Failed to run database migrations")?;

        Ok(PgDataStore { pool })
    }
}

impl DataStore for PgDataStore {
    async fn get_existing_stream_ids(
        &self,
        video_ids: &[&str],
    ) -> anyhow::Result<std::collections::HashSet<String>> {
        #[derive(sqlx::FromRow)]
        struct VideoId {
            video_id: String,
        }

        let streams =
            sqlx::query_as::<_, VideoId>("SELECT video_id FROM streams WHERE video_id = ANY($1)")
                .bind(video_ids)
                .fetch_all(&self.pool)
                .await
                .inspect_err(|e| {
                    tracing::error!(error = ?e, "Failed to fetch existing streams");
                })
                .context("Failed to fetch existing streams")?;

        Ok(streams.into_iter().map(|s| s.video_id).collect())
    }

    async fn insert_stream(&self, stream: &crate::Stream) -> anyhow::Result<()> {
        let timestamp = stream
            .timestamp_from_time_ago()
            .ok_or_else(|| anyhow::anyhow!("Invalid streamed_date: {}", stream.streamed_date))?;

        sqlx::query(
        r#"
            INSERT INTO streams (video_id, title, view_count, stream_timestamp, duration, summary_md, timestamp_md)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT DO NOTHING
            "#
        )
        .bind(&stream.video_id)
        .bind(&stream.title)
        .bind(&stream.view_count)
        .bind(timestamp)
        .bind(&stream.duration)
        .bind(&stream.summary_md)
        .bind(&stream.timestamp_md)
        .execute(&self.pool)
        .await
        .inspect_err(|err| {
            tracing::error!(
                error = ?err,
                video_id = %stream.video_id,
                "Failed to insert stream"
            )
        })
        .context("Failed to insert stream")?;

        Ok(())
    }
}
