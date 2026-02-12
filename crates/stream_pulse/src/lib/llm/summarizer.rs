use std::future::Future;

use serde::{de::DeserializeOwned, Serialize};

pub trait Summarizer {
    const CONTEXT_LIMIT: usize = 128_000 - 18_000;
    const MODEL: &str;

    type ResponseType: DeserializeOwned;
    type Error;

    fn summarize<M: Serialize>(
        &self,
        message_segments: &[M],
    ) -> impl Future<Output = Result<Self::ResponseType, Self::Error>>;
}
