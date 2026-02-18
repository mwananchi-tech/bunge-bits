use std::{path::PathBuf, str::FromStr};

use anyhow::Context;
use apalis::{layers::retry::RetryPolicy, prelude::*};
use apalis_cron::{CronStream, Tick};
use cron::Schedule;
use stream_datastore::PgDataStore;
use ytdlp_bindings::YtDlp;

use stream_pulse::{
    openai::OpenAIClient,
    tracing::init_tracing_subscriber,
    yt::{audio_handler::YtDlpWrapper, scraper::Scraper},
    LiveStreamProcessorBuilder,
};

#[derive(Clone)]
struct Config {
    db_url: String,
    openai_key: String,
    cookies_path: PathBuf,
    max_streams: usize,
}

async fn handle_tick(_tick: Tick, config: Data<Config>) {
    tracing::info!(max_streams = config.max_streams, "Running pipeline...");

    match run_pipeline(&config).await {
        Ok(_) => tracing::info!("Pipeline completed successfully"),
        Err(e) => {
            tracing::error!(error = ?e, "Pipeline failed");
            sentry::capture_error(&*e);
        }
    }
}

async fn run_pipeline(config: &Config) -> anyhow::Result<()> {
    let store = PgDataStore::init(&config.db_url).await?;
    let yt_dlp = YtDlp::new_with_cookies(Some(config.cookies_path.clone()))?;
    let openai = OpenAIClient::new(&config.openai_key, yt_dlp.clone());

    let processor = LiveStreamProcessorBuilder::new("/var/tmp/bunge-bits")
        .store(store)
        .transcriber(openai.clone())
        .summarizer(openai)
        .audio_handler(YtDlpWrapper::new(yt_dlp))
        .channel_scraper(Scraper::default())
        .max_streams(config.max_streams)
        .with_chunking(900)
        .build();

    processor.run().await
}

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

    let config = Config {
        db_url: std::env::var("DATABASE_URL").context("DATABASE_URL not set")?,
        openai_key: std::env::var("OPENAI_API_KEY").context("OPENAI_API_KEY not set")?,
        cookies_path: std::env::var("YTDLP_COOKIES_PATH")
            .map(PathBuf::from)
            .context("YTDLP_COOKIES_PATH not set")?,
        max_streams: std::env::var("MAX_STREAMS_TO_PROCESS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3),
    };

    let schedule = Schedule::from_str(
        &std::env::var("CRON_SCHEDULE").unwrap_or_else(|_| "0 0 */4 * * *".into()),
    )?;

    let worker = WorkerBuilder::new("stream-pulse-cron")
        .backend(CronStream::new(schedule))
        .retry(RetryPolicy::retries(3))
        .data(config)
        .build(handle_tick);

    worker.run().await?;

    Ok(())
}
