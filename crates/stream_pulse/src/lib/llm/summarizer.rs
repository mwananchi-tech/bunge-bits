use std::{fmt::Debug, future::Future};

use serde::Deserialize;

pub trait Summarizer {
    const CONTEXT_WINDOW_LIMIT: usize;
    const SUMMARIZER_MODEL: &'static str;

    type Error: Debug;

    fn summarize(
        &self,
        content: &str,
    ) -> impl Future<Output = Result<SummaryResponse, Self::Error>> + Send + Sync;

    fn count_tokens(&self, _content: &str) -> Result<usize, Self::Error> {
        Ok(0)
    }
}

#[derive(Debug, Deserialize)]
pub struct SummaryResponse {
    // define based on your prompt structure
    pub summary: String,
}
