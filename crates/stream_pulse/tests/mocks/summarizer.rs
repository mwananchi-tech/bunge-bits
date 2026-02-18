use std::sync::{Arc, Mutex};
use stream_pulse::{Summarizer, SummaryResponse};

#[derive(Clone)]
pub struct MockSummarizer {
    pub summary: String,
    pub calls: Arc<Mutex<Vec<String>>>,
    pub fail_with: Option<String>,
}

impl MockSummarizer {
    pub fn new(summary: &str) -> Self {
        Self {
            summary: summary.to_string(),
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_with: None,
        }
    }

    pub fn failing(msg: &str) -> Self {
        Self {
            summary: String::new(),
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_with: Some(msg.to_string()),
        }
    }
}

impl Summarizer for MockSummarizer {
    const CONTEXT_WINDOW_LIMIT: usize = 128_000;
    const SUMMARIZER_MODEL: &'static str = "mock-gpt";
    type Error = anyhow::Error;

    async fn summarize(&self, content: &str) -> Result<SummaryResponse, Self::Error> {
        self.calls.lock().unwrap().push(content.to_string());
        if let Some(ref msg) = self.fail_with {
            return Err(anyhow::anyhow!("{}", msg));
        }
        Ok(SummaryResponse {
            summary: self.summary.clone(),
        })
    }
}
