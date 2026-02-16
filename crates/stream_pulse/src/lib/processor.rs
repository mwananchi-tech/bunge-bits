use std::{fs::remove_dir_all, path::PathBuf};

use anyhow::Context;
use itertools::Itertools;
use rayon::prelude::*;
use stream_datastore::{DataStore, Stream};

use crate::{
    parser::{parse_streams, YtHtmlDocument},
    yt::{AudioHandler, ChannelScraper},
    AudioInput, Summarizer, Transcriber,
};

// The core YouTube archived live stream stream processor
#[derive(Debug)]
pub struct LiveStreamProcessor<D, T, S, A, P>
where
    D: DataStore + Send + Sync + 'static,
    T: Transcriber + Send + Sync + 'static,
    S: Summarizer + Send + Sync + 'static,
    A: AudioHandler + Send + Sync + 'static,
    P: ChannelScraper + Send + Sync + 'static,
{
    workdir: PathBuf,
    store: D,
    transcriber: T,
    summarizer: S,
    audio_handler: A,
    channel_scraper: P,
}

impl<D, T, S, A, P> LiveStreamProcessor<D, T, S, A, P>
where
    D: DataStore + Send + Sync + 'static,
    T: Transcriber + Send + Sync + 'static,
    S: Summarizer + Send + Sync + 'static,
    A: AudioHandler + Send + Sync + 'static,
    P: ChannelScraper + Send + Sync + 'static,
{
    ///  Parliament of Kenya Channel Stream URL
    const YOUTUBE_STREAM_URL: &str = "https://www.youtube.com/@ParliamentofKenyaChannel/streams";
    const YOUTUBE_VIDEO_BASE_URL: &str = "https://youtube.com/watch";

    pub fn new(
        workdir: impl Into<PathBuf>,
        store: D,
        transcriber: T,
        summarizer: S,
        audio_handler: A,
        channel_scraper: P,
    ) -> Self {
        LiveStreamProcessor {
            workdir: workdir.into(),
            store,
            transcriber,
            summarizer,
            audio_handler,
            channel_scraper,
        }
    }

    /// Parses the `ytInitialData` script data from the youtube html document
    #[tracing::instrument(skip_all)]
    async fn parse_streams(&self, doc: &YtHtmlDocument) -> anyhow::Result<Vec<Stream>> {
        let json = doc.to_json::<serde_json::Value>()?;
        let streams = parse_streams(&json)?;
        Ok(streams)
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
        let yt_html_doc = self
            .channel_scraper
            .scrape_channel()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to scrape yt html document: {e:?}"))?;
        let streams = self.parse_streams(&yt_html_doc).await?;

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
                self.audio_handler
                    .download(stream, &audio_dl_path)
                    .and_then(|dl_path| self.audio_handler.clean_up(stream, &dl_path))
                    .map(|processed_audio_path| (processed_audio_path, stream))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        for (audio_path, stream) in stream_audio_paths {
            let audio_input = if should_chunk {
                let chunks_dir_path = workdir_ref.join("audio").join(&stream.video_id);

                // TODO: pass these as configurations
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
                .inspect_err(|e| tracing::error!(error = ?e, "Failed to transcribe audio"))
                .map_err(|e| anyhow::anyhow!("Failed to transcribe audio: {e:?}"))?;

            let summary_resp = self
                .summarizer
                .summarize(&transcribe_resp.text)
                .await
                .inspect_err(|e| tracing::error!(error = ?e, "Failed to summarize transcript"))
                .map_err(|e| anyhow::anyhow!("Failed to summarize transcript: {e:?}"))?;

            stream.summary_md = Some(summary_resp.summary);

            self.store.insert_stream(stream).await?;
        }

        Ok(())
    }
}

impl<D, T, S, A, P> Drop for LiveStreamProcessor<D, T, S, A, P>
where
    D: DataStore + Send + Sync + 'static,
    T: Transcriber + Send + Sync + 'static,
    S: Summarizer + Send + Sync + 'static,
    A: AudioHandler + Send + Sync + 'static,
    P: ChannelScraper + Send + Sync + 'static,
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
