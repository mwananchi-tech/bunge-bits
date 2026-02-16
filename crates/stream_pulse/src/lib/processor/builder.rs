use std::path::PathBuf;

use stream_datastore::DataStore;

use crate::{
    yt::{AudioHandler, ChannelScraper},
    LiveStreamProcessor, Summarizer, Transcriber,
};

#[derive(Debug)]
pub struct ChunkingConfig {
    pub chunk_duration_seconds: u16,
}

pub struct LiveStreamProcessorBuilder<D = (), T = (), S = (), A = (), P = ()> {
    workdir: PathBuf,
    store: D,
    transcriber: T,
    summarizer: S,
    audio_handler: A,
    channel_scraper: P,
    max_streams: usize,
    chunking_config: Option<ChunkingConfig>,
}

impl LiveStreamProcessorBuilder {
    pub fn new(workdir: impl Into<PathBuf>) -> Self {
        Self {
            workdir: workdir.into(),
            store: (),
            transcriber: (),
            summarizer: (),
            audio_handler: (),
            channel_scraper: (),
            max_streams: 5,
            chunking_config: None,
        }
    }
}

impl<D, T, S, A, P> LiveStreamProcessorBuilder<D, T, S, A, P> {
    pub fn store<D2: DataStore + Send + Sync + 'static>(
        self,
        store: D2,
    ) -> LiveStreamProcessorBuilder<D2, T, S, A, P> {
        LiveStreamProcessorBuilder {
            workdir: self.workdir,
            store,
            transcriber: self.transcriber,
            summarizer: self.summarizer,
            audio_handler: self.audio_handler,
            channel_scraper: self.channel_scraper,
            max_streams: self.max_streams,
            chunking_config: self.chunking_config,
        }
    }

    pub fn transcriber<T2: Transcriber + Send + Sync + 'static>(
        self,
        transcriber: T2,
    ) -> LiveStreamProcessorBuilder<D, T2, S, A, P> {
        LiveStreamProcessorBuilder {
            workdir: self.workdir,
            store: self.store,
            transcriber,
            summarizer: self.summarizer,
            audio_handler: self.audio_handler,
            channel_scraper: self.channel_scraper,
            max_streams: self.max_streams,
            chunking_config: self.chunking_config,
        }
    }

    pub fn summarizer<S2: Summarizer + Send + Sync + 'static>(
        self,
        summarizer: S2,
    ) -> LiveStreamProcessorBuilder<D, T, S2, A, P> {
        LiveStreamProcessorBuilder {
            workdir: self.workdir,
            store: self.store,
            transcriber: self.transcriber,
            summarizer,
            audio_handler: self.audio_handler,
            channel_scraper: self.channel_scraper,
            max_streams: self.max_streams,
            chunking_config: self.chunking_config,
        }
    }

    pub fn audio_handler<A2: AudioHandler + Send + Sync + 'static>(
        self,
        audio_handler: A2,
    ) -> LiveStreamProcessorBuilder<D, T, S, A2, P> {
        LiveStreamProcessorBuilder {
            workdir: self.workdir,
            store: self.store,
            transcriber: self.transcriber,
            summarizer: self.summarizer,
            audio_handler,
            channel_scraper: self.channel_scraper,
            max_streams: self.max_streams,
            chunking_config: self.chunking_config,
        }
    }

    pub fn channel_scraper<P2: ChannelScraper + Send + Sync + 'static>(
        self,
        channel_scraper: P2,
    ) -> LiveStreamProcessorBuilder<D, T, S, A, P2> {
        LiveStreamProcessorBuilder {
            workdir: self.workdir,
            store: self.store,
            transcriber: self.transcriber,
            summarizer: self.summarizer,
            audio_handler: self.audio_handler,
            channel_scraper,
            max_streams: self.max_streams,
            chunking_config: self.chunking_config,
        }
    }

    pub fn max_streams(mut self, max_streams: usize) -> Self {
        self.max_streams = max_streams;
        self
    }

    pub fn with_chunking(mut self, chunk_duration_seconds: u16) -> Self {
        self.chunking_config = Some(ChunkingConfig {
            chunk_duration_seconds,
        });
        self
    }
}

impl<D, T, S, A, P> LiveStreamProcessorBuilder<D, T, S, A, P>
where
    D: DataStore + Send + Sync + 'static,
    T: Transcriber + Send + Sync + 'static,
    S: Summarizer + Send + Sync + 'static,
    A: AudioHandler + Send + Sync + 'static,
    P: ChannelScraper + Send + Sync + 'static,
{
    pub fn build(self) -> LiveStreamProcessor<D, T, S, A, P> {
        LiveStreamProcessor {
            workdir: self.workdir,
            store: self.store,
            transcriber: self.transcriber,
            summarizer: self.summarizer,
            audio_handler: self.audio_handler,
            channel_scraper: self.channel_scraper,
            max_streams: self.max_streams,
            chunking_config: self.chunking_config,
        }
    }
}
