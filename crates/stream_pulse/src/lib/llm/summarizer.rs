use std::future::Future;

use serde::{de::DeserializeOwned, Serialize};

pub trait Summarizer {
    const CONTEXT_WINDOW_LIMIT: usize = 128_000 - 18_000;
    const SUMMARIZER_MODEL: &str;

    type ResponseType: DeserializeOwned;
    type Error;

    fn summarize<M: Serialize>(
        &self,
        content: impl Into<String>,
    ) -> impl Future<Output = Result<Self::ResponseType, Self::Error>>;
}
