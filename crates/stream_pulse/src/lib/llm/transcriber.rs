use std::{future::Future, path::PathBuf};

use serde::de::DeserializeOwned;

pub trait Transcriber {
    const MODEL_NAME: &'static str;

    type Response: DeserializeOwned;
    type Error;

    fn transcribe(
        &self,
        audio_input: AudioInput,
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send;
}

pub enum AudioInput {
    Chunked {
        chunk_len_seconds: usize,
        chunks_dir_path: PathBuf,
        file_path: PathBuf,
    },
    File(PathBuf),
}
