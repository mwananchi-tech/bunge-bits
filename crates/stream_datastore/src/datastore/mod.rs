use std::{collections::HashSet, fmt::Debug, future::Future};

use crate::Stream;

pub mod postgres;

pub trait DataStore {
    fn get_existing_stream_ids(
        &self,
        video_ids: &[&str],
    ) -> impl Future<Output = anyhow::Result<HashSet<String>>> + Send;

    fn insert_stream(&self, stream: &Stream) -> impl Future<Output = Result<(), anyhow::Error>>;
}

impl<T: DataStore + Send + Sync> DataStore for &T {
    async fn get_existing_stream_ids(
        &self,
        video_ids: &[&str],
    ) -> anyhow::Result<std::collections::HashSet<String>> {
        (**self).get_existing_stream_ids(video_ids).await
    }

    async fn insert_stream(&self, stream: &Stream) -> Result<(), anyhow::Error> {
        (**self).insert_stream(stream).await
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
