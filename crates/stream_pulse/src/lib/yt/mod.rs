pub mod audio_handler;
pub mod scraper;

use std::{
    fmt::Debug,
    future::Future,
    path::{Path, PathBuf},
};

use stream_datastore::Stream;

use crate::parser::YtHtmlDocument;

pub trait AudioHandler {
    const BASE_URL: &str;

    fn download(&self, stream: &Stream, audio_dl_path: &Path) -> anyhow::Result<PathBuf>;

    fn clean_up(&self, stream: &Stream, audio_dl_path: &Path) -> anyhow::Result<PathBuf>;
}

pub trait ChannelScraper {
    const CHANNEL_URL: &str;

    type Error: Debug;

    fn scrape_channel(&self) -> impl Future<Output = anyhow::Result<YtHtmlDocument>>;
}
