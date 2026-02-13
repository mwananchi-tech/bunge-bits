use std::{collections::HashSet, future::Future};

pub mod postgres;

pub trait DataStore {
    fn get_existing_stream_ids(
        &self,
        video_ids: &[&str],
    ) -> impl Future<Output = anyhow::Result<HashSet<String>>> + Send;

    fn bulk_insert_streams(
        &self,
        streams: &[crate::Stream],
    ) -> impl Future<Output = anyhow::Result<BulkInsertResult>> + Send;
}

impl<T: DataStore + Send + Sync> DataStore for &T {
    async fn get_existing_stream_ids(
        &self,
        video_ids: &[&str],
    ) -> anyhow::Result<std::collections::HashSet<String>> {
        (**self).get_existing_stream_ids(video_ids).await
    }

    async fn bulk_insert_streams(
        &self,
        streams: &[crate::Stream],
    ) -> anyhow::Result<BulkInsertResult> {
        (**self).bulk_insert_streams(streams).await
    }
}

#[derive(Debug)]
pub struct BulkInsertResult {
    pub successful_inserts: usize,
    pub failed_inserts: Vec<FailedInsert>,
}

#[derive(Debug)]
pub struct FailedInsert {
    pub video_id: String,
    pub reason: InsertFailReason,
}

#[derive(Debug)]
pub enum InsertFailReason {
    InvalidStreamedDate { malformed_date: String },
}
