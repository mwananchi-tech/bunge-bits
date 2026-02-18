use std::sync::{Arc, Mutex};
use stream_pulse::{AudioInput, TranscribeResponse, Transcriber};

#[derive(Clone)]
pub struct MockTranscriber {
    pub response_text: String,
    pub calls: Arc<Mutex<Vec<AudioInput>>>,
    pub fail_with: Option<String>,
}

impl MockTranscriber {
    pub fn new(response_text: &str) -> Self {
        Self {
            response_text: response_text.to_string(),
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_with: None,
        }
    }

    pub fn failing(msg: &str) -> Self {
        Self {
            response_text: String::new(),
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_with: Some(msg.to_string()),
        }
    }
}

impl Transcriber for MockTranscriber {
    const TRANSCRIBER_MODEL: &'static str = "mock-whisper";
    type Error = anyhow::Error;

    async fn transcribe(&self, audio_input: AudioInput) -> Result<TranscribeResponse, Self::Error> {
        self.calls.lock().unwrap().push(audio_input);
        if let Some(ref msg) = self.fail_with {
            return Err(anyhow::anyhow!("{}", msg));
        }
        Ok(TranscribeResponse {
            duration: 120.0,
            text: self.response_text.clone(),
            segments: None,
        })
    }
}
