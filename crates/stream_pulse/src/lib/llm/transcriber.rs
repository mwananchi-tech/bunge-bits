use std::{future::Future, path::PathBuf, u16};

use serde::de::DeserializeOwned;

pub trait Transcriber {
    const TRANSCRIPTION_MODEL: &'static str;

    type Response: DeserializeOwned;
    type Error;

    fn transcribe(
        &self,
        audio_input: AudioInput,
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send + Sync;
}

pub enum AudioInput {
    Chunked {
        chunk_duration_seconds: u16,
        chunks_dir_path: PathBuf,
        file_path: PathBuf,
    },
    File(PathBuf),
}
