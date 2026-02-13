use std::path::PathBuf;

use reqwest::Client;
use serde::Deserialize;
use ytdlp_bindings::AudioProcessor;

use crate::{AudioInput, Summarizer, Transcriber};

pub struct OpenAIClient<F: AudioProcessor> {
    client: Client,
    api_key: String,
    ffmpeg: F,
    base_url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenAIError {
    #[error("HTTP error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },
    #[error("FFmpeg error: {0}")]
    Ffmpeg(String),
    #[error("Unsupported input: OpenAI transcriber only supports chunked input")]
    UnsupportedInput,
}

impl<F: AudioProcessor> OpenAIClient<F> {
    const SYSTEM_PROMPT: &str = include_str!("./prompts/system_0.txt");

    pub fn new(api_key: impl Into<String>, ffmpeg: F) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1".into(),
            ffmpeg,
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub async fn send_transcribe_request(
        &self,
        file: impl Into<PathBuf>,
        model_name: impl Into<String>,
        prompt: Option<String>,
    ) -> Result<TranscribeResponse, OpenAIError> {
        let audio_path = file.into();

        let bytes = tokio::fs::read(&audio_path).await?;
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name("chunk.mp3")
            .mime_str("audio/mpeg")
            .unwrap();

        let mut form = reqwest::multipart::Form::new()
            .text("model", model_name.into())
            .text("response_format", "verbose_json")
            .text("timestamp_granularities[]", "segment")
            .part("file", part);

        if let Some(prompt) = prompt {
            form = form.text("prompt", prompt);
        }

        let resp = self
            .client
            .post(format!("{}/audio/transcriptions", self.base_url))
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .inspect_err(|e| tracing::error!(error = %e, "Failed to make http request"))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let message = resp.text().await.unwrap_or_default();
            return Err(OpenAIError::Api { status, message });
        }

        let response = resp.json::<TranscribeResponse>().await?;

        Ok(response)
    }

    pub async fn send_completion_request(
        &self,
        model_name: impl Into<String>,
        user_content: impl Into<String>,
    ) -> Result<CompletionResponse, OpenAIError> {
        let body = serde_json::json!({
            "model": model_name.into(),
            "web_search_options": {
                "search_context_size": "medium",
                "user_location": {
                    "type": "approximate",
                    "approximate": {
                        "country": "KE",
                        "city": "Nairobi",
                        "region": "Nairobi"
                    }
                }
            },
            "messages": [
                {
                    "role": "system",
                    "content": Self::SYSTEM_PROMPT
                },
                {
                    "role": "user",
                    "content": user_content.into()
                }
            ]
        });

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .inspect_err(|e| tracing::error!(error = %e, "Failed to make http request"))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let message = resp.text().await.unwrap_or_default();
            return Err(OpenAIError::Api { status, message });
        }

        Ok(resp.json::<CompletionResponse>().await?)
    }
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

#[derive(Debug, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub choices: Vec<CompletionChoice>,
}

#[derive(Debug, Deserialize)]
pub struct CompletionChoice {
    pub index: u32,
    pub message: CompletionMessage,
    pub finish_reason: String,
}

#[derive(Debug, Deserialize)]
pub struct CompletionMessage {
    pub role: String,
    pub content: Option<String>,
}

impl<F: AudioProcessor + Send + Sync> Transcriber for OpenAIClient<F> {
    const TRANSCRIPTION_MODEL: &'static str = "whisper-1";
    type Response = TranscribeResponse;
    type Error = OpenAIError;

    async fn transcribe(&self, input: AudioInput) -> Result<Self::Response, Self::Error> {
        let AudioInput::Chunked {
            file_path,
            chunks_dir_path,
            chunk_duration_seconds,
        } = input
        else {
            tracing::error!(audio_input = ?input, "Unspoorted audio_input");
            return Err(OpenAIError::UnsupportedInput);
        };

        let chunks_exist = std::fs::read_dir(&chunks_dir_path)
            .map(|mut entries| entries.any(|e| e.is_ok()))
            .unwrap_or(false);

        // chunk via ffmpeg if not already done
        if !chunks_exist {
            std::fs::create_dir_all(&chunks_dir_path)?;
            let base_name = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| OpenAIError::Ffmpeg("Invalid file path".into()))?;

            tracing::info!("Splitting audio to chunks");
            self.ffmpeg
                .split_audio_to_chunks(
                    &file_path,
                    chunk_duration_seconds,
                    chunks_dir_path.join(format!("{base_name}_%03d.mp3")),
                )
                .inspect_err(|e| tracing::error!(error = %e, "Failed to split audio to chunks"))
                .map_err(|e| OpenAIError::Ffmpeg(e.to_string()))?;
        }

        // collect and sort chunk files
        let mut chunks: Vec<PathBuf> = std::fs::read_dir(&chunks_dir_path)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        chunks.sort();

        let mut all_segments = Vec::new();
        let mut all_text = String::new();
        let mut time_offset = 0.0_f64;
        let mut duration = 0.0_f64;
        let mut previous_text = None;

        for chunk in &chunks {
            let response = self
                .send_transcribe_request(chunk, Self::TRANSCRIPTION_MODEL, previous_text)
                .await
                .inspect_err(|e| tracing::error!(error = %e, "Failed to transcribe audio"))?;

            duration += response.duration;

            if let Some(segments) = response.segments {
                for mut seg in segments {
                    seg.start += time_offset;
                    seg.end += time_offset;
                    all_segments.push(seg);
                }
            }

            all_text.push_str(&response.text);
            all_text.push(' ');
            previous_text = Some(response.text);
            time_offset += chunk_duration_seconds as f64;
        }

        Ok(TranscribeResponse {
            duration,
            text: all_text.trim().to_string(),
            segments: Some(all_segments),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct SummaryResponse {
    // define based on your prompt structure
    pub summary: String,
}

impl<F: AudioProcessor> Summarizer for OpenAIClient<F> {
    const SUMMARIZER_MODEL: &'static str = "gpt-4o-search-preview";
    type ResponseType = SummaryResponse;
    type Error = OpenAIError;

    async fn summarize<M: serde::Serialize>(
        &self,
        content: impl Into<String>,
    ) -> Result<Self::ResponseType, Self::Error> {
        let response = self
            .send_completion_request(Self::SUMMARIZER_MODEL, content)
            .await
            .inspect_err(|e| tracing::error!(error = %e, "Failed to summarize content"))?;

        let summary = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| OpenAIError::Api {
                status: 0,
                message: "No conent in response".into(),
            })?;

        Ok(SummaryResponse { summary })
    }
}
