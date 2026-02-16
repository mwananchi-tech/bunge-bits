use std::{fmt::Debug, future::Future, path::PathBuf};

use serde::Deserialize;

pub trait Transcriber {
    const TRANSCRIBER_MODEL: &'static str;

    type Error: Debug;

    fn transcribe(
        &self,
        audio_input: AudioInput,
    ) -> impl Future<Output = Result<TranscribeResponse, Self::Error>> + Send + Sync;
}

#[derive(Debug, Clone)]
pub enum AudioInput {
    Chunked {
        chunk_duration_seconds: u16,
        chunks_dir_path: PathBuf,
        file_path: PathBuf,
    },
    File(PathBuf),
}

#[derive(Debug, Deserialize)]
pub struct TranscribeResponse {
    pub duration: f64,
    pub text: String,
    pub segments: Option<Vec<TranscribeSegment>>,
}

#[derive(Debug, Deserialize)]
pub struct TranscribeSegment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}
