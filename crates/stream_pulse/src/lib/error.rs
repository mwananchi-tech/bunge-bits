#[allow(clippy::enum_variant_names)]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("ParseError: {0}")]
    ParseError(&'static str),
    #[error("Deserialization Error: {0}")]
    DeserializationError(#[from] serde_json::Error),
    #[error(transparent)]
    InternalError(#[from] anyhow::Error),
    #[error("Failed to transcribe audio: {0}")]
    TranscribeError(anyhow::Error),
}
