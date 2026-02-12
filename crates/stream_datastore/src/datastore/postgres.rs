use std::sync::LazyLock;

use anyhow::Context;
use itertools::{Either, Itertools};
use sqlx::{migrate::Migrator, postgres::PgPoolOptions, PgPool};

use super::{BulkInsertResult, FailedInsert, InsertFailReason};
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

    async fn bulk_insert_streams(
        &self,
        streams: &[crate::Stream],
    ) -> anyhow::Result<BulkInsertResult> {
        let (valid_streams, invalid_stream_date_errors): (Vec<_>, Vec<_>) =
            streams.iter().partition_map(|stream| {
                if let Some(timestamp) = stream.timestamp_from_time_ago() {
                    Either::Left((stream.clone(), timestamp))
                } else {
                    let reason = InsertFailReason::InvalidStreamedDate {
                        malformed_date: stream.streamed_date.clone(),
                    };
                    Either::Right(FailedInsert {
                        video_id: stream.video_id.clone(),
                        reason,
                    })
                }
            });

        let (video_ids, title, view_counts, streamed_dates, durations, summaries, timestamp_md): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = valid_streams
            .iter()
            .map(|(stream, stream_date)| {
                (
                    stream.video_id.clone(),
                    stream.title.clone(),
                    stream.view_count.clone(),
                    *stream_date,
                    stream.duration.clone(),
                    stream.summary_md.clone(),
                    stream.timestamp_md.clone(),
                )
            })
            .multiunzip();

        let pg_result = sqlx::query(
            "
            INSERT INTO streams (video_id, title, view_count,stream_timestamp, duration, summary_md, timestamp_md)
            SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::timestamptz[], $5::text[], $6::text[], $7::text[]) ON CONFLICT DO NOTHING
            "
        )
        .bind(&video_ids[..])
        .bind(&title[..])
        .bind(&view_counts[..])
        .bind(&streamed_dates[..])
        .bind(&durations[..])
        .bind(&summaries[..])
        .bind(&timestamp_md[..])
        .execute(&self.pool)
        .await
        .inspect_err(|err| {
            tracing::error!(
                error = ?err,
                "Failed to execute bulk insert for streams"
            )
        })
        .context("Failed to execute bulk insert for streams")?;

        let successful_inserts = pg_result.rows_affected() as usize;

        if !invalid_stream_date_errors.is_empty() {
            tracing::warn!(
                invalid_stream_date_errors = ?invalid_stream_date_errors,
                "Some streams had invalid streamed_date formats and were not inserted"
            )
        }

        Ok(BulkInsertResult {
            successful_inserts,
            failed_inserts: invalid_stream_date_errors,
        })
    }
}
