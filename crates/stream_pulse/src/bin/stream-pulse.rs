use std::{path::PathBuf, str::FromStr};

use apalis::{
    layers::{retry::RetryPolicy, sentry::SentryLayer},
    prelude::*,
};
use apalis_cron::{CronStream, Tick};
use clap::{Parser, Subcommand};
use cron::Schedule;
use stream_datastore::PgDataStore;
use stream_pulse::{
    openai::OpenAIClient,
    tracing::init_tracing_subscriber,
    yt::{audio_handler::YtDlpWrapper, scraper::Scraper},
    LiveStreamProcessorBuilder,
};
use ytdlp_bindings::YtDlp;

#[derive(Parser)]
#[command(name = "stream-pulse", about = "Kenyan Parliament stream processor")]
struct Cli {
    /// Database connection URL
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,

    /// OpenAI API key
    #[arg(long, env = "OPENAI_API_KEY")]
    openai_key: String,

    /// Path to yt-dlp cookies file
    #[arg(long, env = "YTDLP_COOKIES_PATH")]
    cookies_path: PathBuf,

    /// Maximum streams to process per run
    #[arg(long, env = "MAX_STREAMS_TO_PROCESS", default_value = "3")]
    max_streams: usize,

    /// Audio chunk duration in seconds
    #[arg(long, default_value = "900")]
    chunk_duration: u16,

    /// Working directory for audio files
    #[arg(long, default_value = "/var/tmp/bunge-bits")]
    workdir: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the pipeline once and exit
    Run,
    /// Start the cron scheduler
    Cron {
        /// Cron schedule expression
        #[arg(long, env = "CRON_SCHEDULE", default_value = "0 0 */4 * * *")]
        schedule: String,
    },
}

#[derive(Clone)]
struct Config {
    db_url: String,
    openai_key: String,
    cookies_path: PathBuf,
    max_streams: usize,
    chunk_duration: u16,
    workdir: PathBuf,
}

async fn run_pipeline(config: &Config) -> anyhow::Result<()> {
    let store = PgDataStore::init(&config.db_url).await?;
    let yt_dlp = YtDlp::new_with_cookies(Some(config.cookies_path.clone()))?;
    let openai = OpenAIClient::new(&config.openai_key, yt_dlp.clone());

    let processor = LiveStreamProcessorBuilder::new(&config.workdir)
        .store(store)
        .transcriber(openai.clone())
        .summarizer(openai)
        .audio_handler(YtDlpWrapper::new(yt_dlp))
        .channel_scraper(Scraper::default())
        .max_streams(config.max_streams)
        .with_chunking(config.chunk_duration)
        .build();

    processor.run().await
}

async fn handle_tick(_tick: Tick, config: Data<Config>) -> anyhow::Result<()> {
    tracing::info!(
        max_streams = config.max_streams,
        "Running scheduled pipeline..."
    );
    run_pipeline(&config).await
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

    let cli = Cli::parse();
    init_tracing_subscriber()?;

    let config = Config {
        db_url: cli.database_url,
        openai_key: cli.openai_key,
        cookies_path: cli.cookies_path,
        max_streams: cli.max_streams,
        chunk_duration: cli.chunk_duration,
        workdir: cli.workdir,
    };

    match cli.command {
        Command::Run => {
            tracing::info!(max_streams = config.max_streams, "Running pipeline once...");
            run_pipeline(&config).await?;
        }
        Command::Cron { schedule } => {
            tracing::info!(%schedule, "Starting cron scheduler...");
            let schedule = Schedule::from_str(&schedule)?;

            let worker = WorkerBuilder::new("stream-pulse-cron")
                .backend(CronStream::new(schedule))
                .retry(RetryPolicy::retries(3))
                .layer(SentryLayer::new())
                .data(config)
                .build(handle_tick);

            worker.run().await?;
        }
    }

    Ok(())
}
