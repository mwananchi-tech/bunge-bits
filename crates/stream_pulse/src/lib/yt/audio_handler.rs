use std::ops::Deref;

use ytdlp_bindings::{AudioProcessor, YtDlp};

use crate::yt::AudioHandler;

pub struct YtDlpWrapper(pub YtDlp);

impl Deref for YtDlpWrapper {
    type Target = YtDlp;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AudioHandler for YtDlpWrapper {
    const BASE_URL: &str = "https://youtube.com/watch";

    fn download(
        &self,
        stream: &stream_datastore::Stream,
        audio_dl_path: &std::path::Path,
    ) -> anyhow::Result<std::path::PathBuf> {
        let stream_url = format!("{}?v={}", Self::BASE_URL, stream.video_id);

        let base_name = &stream.video_id;
        let audio_output_template = audio_dl_path.join(format!("{base_name}.%(ext)s"));
        let audio_mp3_path = audio_dl_path.join(format!("{base_name}.mp3"));

        // download audio if needed
        if !audio_mp3_path.exists() {
            if let Err(e) = self
                .download_audio(&stream_url, "mp3", &audio_output_template)
                .inspect_err(|e| tracing::error!(error = ?e, "Failed to download audio"))
            {
                anyhow::bail!("Failed to download audio: {:?}", e);
            }

            if !audio_mp3_path.exists() {
                anyhow::bail!(
                    "yt-dlp did not produce expected file: {}",
                    audio_mp3_path.display()
                );
            }
        } else {
            tracing::debug!("Audio already exists at {}", audio_mp3_path.display());
        }
        Ok(audio_mp3_path)
    }

    fn clean_up(
        &self,
        stream: &stream_datastore::Stream,
        audio_dl_path: &std::path::Path,
    ) -> anyhow::Result<std::path::PathBuf> {
        // intermediate cleaned file paths
        let base_name = &stream.video_id;
        let audio_mp3_path = audio_dl_path.join(format!("{base_name}.mp3"));

        let denoised_path = audio_dl_path.join(format!("{base_name}_denoised.mp3"));
        let normalized_path = audio_dl_path.join(format!("{base_name}_normalized.mp3"));
        let trimmed_path = audio_dl_path.join(format!("{base_name}_trimmed.mp3"));

        // perform cleanup if final trimmed audio does not exist
        if !trimmed_path.exists() {
            self.denoise_audio(audio_mp3_path, &denoised_path)
                .and_then(|_| self.normalize_volume(&denoised_path, &normalized_path))
                .and_then(|_| self.trim_silence(&normalized_path, &trimmed_path))?;
        } else {
            tracing::debug!("Cleaned audio already exists at {:?}", trimmed_path);
        }
        Ok(trimmed_path)
    }
}
