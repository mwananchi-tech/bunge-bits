use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use stream_datastore::Stream;
use stream_pulse::yt::AudioHandler;

#[derive(Clone)]
pub struct MockAudioHandler {
    pub calls: Arc<Mutex<Vec<String>>>,
    pub fail_with: Option<String>,
}

impl Default for MockAudioHandler {
    fn default() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_with: None,
        }
    }
}

impl MockAudioHandler {
    pub fn failing(msg: &str) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_with: Some(msg.to_string()),
        }
    }
}

impl AudioHandler for MockAudioHandler {
    const BASE_URL: &'static str = "https://youtube.com";

    fn download(&self, stream: &Stream, _audio_dl_path: &Path) -> anyhow::Result<PathBuf> {
        if let Some(ref msg) = self.fail_with {
            return Err(anyhow::anyhow!("{}", msg));
        }
        self.calls.lock().unwrap().push(stream.video_id.clone());
        Ok(PathBuf::from(format!("/tmp/mock/{}.mp3", stream.video_id)))
    }

    fn clean_up(&self, _stream: &Stream, audio_dl_path: &Path) -> anyhow::Result<PathBuf> {
        Ok(audio_dl_path.to_path_buf())
    }
}
