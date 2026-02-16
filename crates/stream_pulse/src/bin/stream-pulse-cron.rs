use std::path::PathBuf;

use anyhow::Context;
use stream_datastore::PgDataStore;
use ytdlp_bindings::YtDlp;

use stream_pulse::{
    openai::OpenAIClient,
    tracing::init_tracing_subscriber,
    yt::{audio_handler::YtDlpWrapper, scraper::Scraper},
    LiveStreamProcessorBuilder,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let _guard = sentry::init((
        std::env::var("SENTRY_DSN").unwrap_or_default(),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some("production".into()),
            ..Default::default()
        },
    ));

    init_tracing_subscriber()?;

    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL not set")?;
    let openai_key = std::env::var("OPENAI_API_KEY").context("OPENAI_API_KEY not set")?;
    let cookies_path = std::env::var("YTDLP_COOKIES_PATH")
        .map(PathBuf::from)
        .context("YTDLP_COOKIES_PATH not set")?;

    let store = PgDataStore::init(&db_url).await?;
    let yt_dlp = YtDlp::new_with_cookies(Some(cookies_path))?;

    //XXX: handles both transcription and summarization; hence will need to be cloned
    let openai_client = OpenAIClient::new(&openai_key, yt_dlp.clone());
    let audio_handler = YtDlpWrapper::new(yt_dlp.clone());
    let channel_scraper = Scraper::default();

    let processor = LiveStreamProcessorBuilder::new("/var/tmp/bunge-bits")
        .store(store)
        .transcriber(openai_client.clone())
        .summarizer(openai_client)
        .audio_handler(audio_handler)
        .channel_scraper(channel_scraper)
        .max_streams(5)
        .build();

    processor.run().await?;

    Ok(())
}
