use std::{
    fs::remove_dir_all,
    path::{Path, PathBuf},
};

use anyhow::Context;
use itertools::Itertools;
use rayon::prelude::*;
use stream_datastore::{DataStore, Stream};
use ytdlp_bindings::{AudioProcessor, YtDlp};

use crate::{
    parser::{parse_streams, YtHtmlDocument},
    AudioInput, Summarizer, Transcriber,
};

// The core YouTube archived live stream stream processor
#[derive(Debug)]
pub struct LiveStreamProcessor<D, T, S>
where
    D: DataStore + Send + Sync + 'static,
    T: Transcriber + Send + Sync + 'static,
    S: Summarizer + Send + Sync + 'static,
{
    workdir: PathBuf,
    http_client: reqwest::Client,
    yt_dlp: YtDlp,
    store: D,
    transcriber: T,
    summarizer: S,
}

impl<D, T, S> LiveStreamProcessor<D, T, S>
where
    D: DataStore + Send + Sync + 'static,
    T: Transcriber + Send + Sync + 'static,
    S: Summarizer + Send + Sync + 'static,
{
    ///  Parliament of Kenya Channel Stream URL
    const YOUTUBE_STREAM_URL: &str = "https://www.youtube.com/@ParliamentofKenyaChannel/streams";
    const YOUTUBE_VIDEO_BASE_URL: &str = "https://youtube.com/watch";

    pub fn new(
        workdir: impl Into<PathBuf>,
        yt_dlp: YtDlp,
        store: D,
        transcriber: T,
        summarizer: S,
    ) -> Self {
        LiveStreamProcessor {
            workdir: workdir.into(),
            http_client: reqwest::Client::new(),
            yt_dlp,
            store,
            transcriber,
            summarizer,
        }
    }

    /// Loads the youtube streams html page
    #[tracing::instrument(skip(self))]
    async fn fetch_yt_html_document(&self) -> anyhow::Result<YtHtmlDocument> {
        let yt_html_document = self
            .http_client
            .get(Self::YOUTUBE_STREAM_URL)
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await?
            .text()
            .await?;

        Ok(yt_html_document.into())
    }

    /// Parses the `ytInitialData` script data from the youtube html document
    #[tracing::instrument(skip_all)]
    async fn parse_streams(&self, doc: &YtHtmlDocument) -> anyhow::Result<Vec<Stream>> {
        let json = doc.to_json::<serde_json::Value>()?;
        let streams = parse_streams(&json)?;
        Ok(streams)
    }

    /// Downloads youtube video via `yt_dlp` and stores it in `audio_dl_path`
    #[tracing::instrument(skip(self))]
    fn download_audio(&self, stream: &Stream, audio_dl_path: &Path) -> anyhow::Result<PathBuf> {
        let stream_url = format!("{}?v={}", Self::YOUTUBE_VIDEO_BASE_URL, stream.video_id);

        let base_name = &stream.video_id;
        let audio_output_template = audio_dl_path.join(format!("{base_name}.%(ext)s"));
        let audio_mp3_path = audio_dl_path.join(format!("{base_name}.mp3"));

        // download audio if needed
        if !audio_mp3_path.exists() {
            if let Err(e) = self
                .yt_dlp
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

    /// Performs cleanup operations of the downloaded audio in `audio_dl_path`
    /// Returns the path of the final cleaned audio path
    #[tracing::instrument(skip(self))]
    fn process_audio(&self, stream: &Stream, audio_dl_path: &Path) -> anyhow::Result<PathBuf> {
        // intermediate cleaned file paths
        let base_name = &stream.video_id;
        let audio_mp3_path = audio_dl_path.join(format!("{base_name}.mp3"));

        let denoised_path = audio_dl_path.join(format!("{base_name}_denoised.mp3"));
        let normalized_path = audio_dl_path.join(format!("{base_name}_normalized.mp3"));
        let trimmed_path = audio_dl_path.join(format!("{base_name}_trimmed.mp3"));

        // perform cleanup if final trimmed audio does not exist
        if !trimmed_path.exists() {
            self.yt_dlp
                .denoise_audio(audio_mp3_path, &denoised_path)
                .and_then(|_| {
                    self.yt_dlp
                        .normalize_volume(&denoised_path, &normalized_path)
                })
                .and_then(|_| self.yt_dlp.trim_silence(&normalized_path, &trimmed_path))?;
        } else {
            tracing::debug!("Cleaned audio already exists at {:?}", trimmed_path);
        }
        Ok(trimmed_path)
    }

    #[tracing::instrument(skip_all)]
    async fn sort_filter_limit_streams(
        &self,
        streams: Vec<Stream>,
        max_streams: usize,
    ) -> anyhow::Result<Vec<Stream>> {
        let stream_ids = streams
            .iter()
            .map(|s| s.video_id.as_str())
            .collect::<Vec<_>>();
        let existing_stream_ids = self
            .store
            .get_existing_stream_ids(&stream_ids)
            .await
            .inspect_err(|e| {
                tracing::error!(error = ?e, "Failed to get existing stream IDs");
            })
            .context("Failed to get existing stream IDs")?;

        let result = streams
            .iter()
            .filter(|s| !existing_stream_ids.contains(&s.video_id))
            // sort filtered streams by timestamp ascending (older streams first)
            // newer streams will “wait their turn” behind older unprocessed ones.
            .sorted_by(|a, b| {
                a.timestamp_from_time_ago()
                    .cmp(&b.timestamp_from_time_ago())
            })
            // return the first `max_streams` streams to avoid overloading system
            .take(max_streams)
            .cloned()
            .collect::<Vec<_>>();

        Ok(result)
    }

    #[tracing::instrument(skip(self))]
    pub async fn run(self, max_streams: usize, should_chunk: bool) -> anyhow::Result<()> {
        let yt_html_doc = self.fetch_yt_html_document().await?;

        let streams = self.parse_streams(&yt_html_doc).await?;
        tracing::info!(count = streams.len(), "Processing streams");

        let mut streams = self.sort_filter_limit_streams(streams, max_streams).await?;
        if streams.is_empty() {
            tracing::info!("No streams to process at this time");
            return Ok(());
        }

        let workdir_ref = self.workdir.as_path();
        let audio_dl_path = workdir_ref.join("audio");

        let stream_audio_paths = streams
            .par_iter_mut()
            .map(|stream| {
                self.download_audio(stream, &audio_dl_path)
                    .and_then(|dl_path| self.process_audio(stream, &dl_path))
                    .map(|processed_audio_path| (processed_audio_path, stream))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        for (audio_path, stream) in stream_audio_paths {
            let audio_input = if should_chunk {
                let chunks_dir_path = workdir_ref.join("audio").join(&stream.video_id);

                AudioInput::Chunked {
                    chunk_duration_seconds: 900, // 15 * 60 seconds
                    chunks_dir_path,
                    file_path: audio_path,
                }
            } else {
                AudioInput::File(audio_path)
            };
            let transcribe_resp = self
                .transcriber
                .transcribe(audio_input)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to transcribe audio: {e:?}"))?;

            let summary_resp = self
                .summarizer
                .summarize(&transcribe_resp.text)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to summarize transcript: {e:?}"))?;

            stream.summary_md = Some(summary_resp.summary)
            // TODO: Maybe just insert a single stream
        }

        self.store.bulk_insert_streams(&streams).await?;

        Ok(())
    }
}

impl<D, T, S> Drop for LiveStreamProcessor<D, T, S>
where
    D: DataStore + Send + Sync + 'static,
    T: Transcriber + Send + Sync + 'static,
    S: Summarizer + Send + Sync + 'static,
{
    fn drop(&mut self) {
        let workdir_ref = self.workdir.as_path();
        let audio_path = workdir_ref.join("audio");

        if audio_path.exists() {
            if let Err(e) = remove_dir_all(&audio_path) {
                tracing::warn!(error = ?e, path = ?audio_path, "Failed to clean up audio directory");
            } else {
                tracing::info!(path = ?audio_path, "Cleaned up audio directory");
            }
        }
    }
}
