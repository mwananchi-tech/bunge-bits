use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use anyhow::{bail, Context};
use itertools::Itertools;
use rayon::prelude::*;
use regex::Regex;
use serde::Deserialize;
use stream_datastore::{DataStore, Stream};
use ytdlp_bindings::{AudioProcessor, YtDlp};

use crate::parser::{extract_json_from_script, parse_streams};

// Repeated number chains like 1.0-2-1.0-1-1-...
pub static RE_NUMBER_CHAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)(\d+(?:[.\-]\d+){5,})").unwrap());
// Numeric-only garbage lines like "1.0-1-1-1-1-1-1"
pub static RE_NUMERIC_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[\d.\-, ]{10,}$").unwrap());

// The core YouTube archived live stream stream processor
#[derive(Debug)]
pub struct LiveStreamProcessor<D>
where
    D: DataStore + Send + Sync + 'static,
{
    workdir: PathBuf,
    http_client: reqwest::Client,
    yt_dlp: YtDlp,
    store: D,
}

pub struct YtHtmlDocument(String);

impl YtHtmlDocument {
    pub fn to_json<T>(&self) -> Result<T, crate::error::Error>
    where
        T: for<'a> Deserialize<'a>,
    {
        extract_json_from_script(&self.0)
    }
}

impl From<String> for YtHtmlDocument {
    fn from(value: String) -> Self {
        YtHtmlDocument(value)
    }
}

impl<D> LiveStreamProcessor<D>
where
    D: DataStore + Send + Sync + 'static,
{
    ///  Parliament of Kenya Channel Stream URL
    const YOUTUBE_STREAM_URL: &str = "https://www.youtube.com/@ParliamentofKenyaChannel/streams";
    const YOUTUBE_VIDEO_BASE_URL: &str = "https://youtube.com/watch";
    // const TRANSCRIPT_CHUNK_DELIMITER: &str = "----END_OF_CHUNK----";

    pub fn new(workdir: impl Into<PathBuf>, yt_dlp: YtDlp, store: D) -> Self {
        LiveStreamProcessor {
            workdir: workdir.into(),
            http_client: reqwest::Client::new(),
            yt_dlp,
            store,
        }
    }

    /// Loads the youtube streams html page
    async fn fetch_yt_html_document(&self) -> anyhow::Result<YtHtmlDocument> {
        let yt_html_document = self
            .http_client
            .get(LiveStreamProcessor::<D>::YOUTUBE_STREAM_URL)
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await?
            .text()
            .await?;

        let yt_html_document = YtHtmlDocument(yt_html_document);
        Ok(yt_html_document)
    }

    /// Parses the `ytInitialData` script data from the youtube html document
    async fn parse_streams(&self, doc: &YtHtmlDocument) -> anyhow::Result<Vec<Stream>> {
        let json = doc.to_json::<serde_json::Value>()?;
        let streams = parse_streams(&json)?;
        Ok(streams)
    }

    /// Downloads youtube video via `yt_dlp` and stores it in `audio_dl_path`
    fn download_audio(&self, stream: &Stream, audio_dl_path: &Path) -> anyhow::Result<PathBuf> {
        let stream_url = format!(
            "{}?v={}",
            LiveStreamProcessor::<D>::YOUTUBE_VIDEO_BASE_URL,
            stream.video_id
        );

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
                bail!("Failed to download audio: {:?}", e);
            }

            if !audio_mp3_path.exists() {
                bail!(
                    "yt-dlp did not produce expected file: {}",
                    audio_mp3_path.display()
                );
            }
        } else {
            tracing::debug!("Audio already exists at {:?}", audio_mp3_path);
        }
        Ok(audio_mp3_path)
    }

    /// Performs cleanup operations of the downloaded audio in `audio_dl_path`
    /// Returns the path of the final cleaned audio path
    fn process_audio(&self, stream: &Stream, audio_dl_path: &Path) -> anyhow::Result<PathBuf> {
        // intermediate cleaned file paths
        let base_name = &stream.video_id;
        let audio_mp3_path = audio_dl_path.join(format!("{base_name}.mp3"));

        let denoised_path = audio_dl_path.join(format!("{base_name}_denoised.mp3"));
        let normalized_path = audio_dl_path.join(format!("{base_name}_normalized.mp3"));
        let trimmed_path = audio_dl_path.join(format!("{base_name}_trimmed.mp3"));

        // perform cleanup if final trimmed audio does not exist
        if !trimmed_path.exists() {
            self.yt_dlp.denoise_audio(audio_mp3_path, &denoised_path)?;
            self.yt_dlp
                .normalize_volume(&denoised_path, &normalized_path)?;
            self.yt_dlp.trim_silence(&normalized_path, &trimmed_path)?;
        } else {
            tracing::debug!("Cleaned audio already exists at {:?}", trimmed_path);
        }
        Ok(trimmed_path)
    }

    /// Split audio to chunks and save resulting audio to `WORKDIR/audio/<video_id>/` directory
    fn chunk_audio(&self, stream: &Stream, cleaned_audio_path: &Path) -> anyhow::Result<()> {
        let base_name = &stream.video_id;
        let work_dir_ref = self.workdir.as_path();
        let chunked_audio_path = work_dir_ref.join("audio").join(base_name);

        // split if chunks not already present
        let chunks_exists = std::fs::read_dir(&chunked_audio_path)
            .map(|mut entries| entries.any(|e| e.is_ok()))
            .unwrap_or(false);

        if !chunks_exists {
            create_dir_all(&chunked_audio_path)?;
            self.yt_dlp.split_audio_to_chunks(
                cleaned_audio_path,
                900, // 15 * 60 seconds
                chunked_audio_path.join(format!("{base_name}_%03d.mp3")),
            )?;
        } else {
            tracing::debug!("Chunks already exist at {:?}", chunked_audio_path);
        }
        Ok(())
    }

    pub async fn run(self, max_streams: usize) -> anyhow::Result<()> {
        let yt_html_doc = self.fetch_yt_html_document().await?;

        let streams = self.parse_streams(&yt_html_doc).await?;
        tracing::info!(count = streams.len(), "Processing streams");

        let streams = sort_filter_limit_streams(max_streams, &self.store, streams).await?;
        if streams.is_empty() {
            tracing::info!("No streams to process at this time");
        }

        let workdir_ref = self.workdir.as_path();
        let audio_dl_path = workdir_ref.join("audio");

        streams.par_iter().try_for_each(|stream| {
            self.download_audio(stream, &audio_dl_path)
                .and_then(|dl_path| self.process_audio(stream, &dl_path))
                .and_then(|processed_path| self.chunk_audio(stream, &processed_path))
        })?;

        // TODO: Transcribe

        // TODO: Summarize

        Ok(())
    }
}

pub async fn sort_filter_limit_streams(
    max_streams: usize,
    db: impl DataStore,
    streams: Vec<Stream>,
) -> anyhow::Result<Vec<Stream>> {
    let stream_ids = streams
        .iter()
        .map(|s| s.video_id.as_str())
        .collect::<Vec<_>>();
    let existing_stream_ids = db
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
