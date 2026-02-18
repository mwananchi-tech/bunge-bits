# stream_pulse

The core engine powering `bunge-bits`. This crate handles all logic related to the summarization pipeline.

## Development setup

To run the `stream-pulse` binary, set the following environment variables:

```bash
OPENAI_API_KEY="<your_openai_api_key>"
DATABASE_URL="<your_postgres_database_url>"
YTDLP_COOKIES_PATH="<path to your cookies.txt file>" # required in order to authenticate to yt, especially in a cloud env
SENTRY_DSN="<optional_sentry_dsn>" # can be omitted for local development
MAX_STREAMS_TO_PROCESS=3 # optional config of the maximum number of streams that can be processed in a given run
CRON_SCHEDULE="<cron_expression>" # optional cron schedule to run the pipeline. Defaults to "0 0 */4 * * *" (every 4 hours)
```

Please read [this guide](../ytdlp_bindings/README.md#using-cookiestxt-for-authenticated-youtube-downloads) on how to setup your `cookies.txt` file.

## Running the CLI

For quick local testing and one-off runs:

```bash
cargo run --bin stream-pulse -- run --max-streams 2
```

The `--max-streams` flag is optional (default: 3). This runs the pipeline once and exits.

## Running the Cron Scheduler

To start the scheduled production workflow:

```bash
cargo run --bin stream-pulse -- cron
```

With a custom schedule:

```bash
cargo run --bin stream-pulse -- cron --schedule "0 */30 * * * *"
```

## Running with Docker

To run `stream-pulse` reliably with environment configuration and persistent file storage:

```bash
docker run -d \
  --name stream-pulse \
  --restart unless-stopped \
  -e OPENAI_API_KEY="..." \
  -e DATABASE_URL="..." \
  -e SENTRY_DSN="..." \
  -e CRON_SCHEDULE="..." \
  -e MAX_STREAMS_TO_PROCESS=2 \
  -e YTDLP_COOKIES_PATH=/app/cookies.txt \
  -v /path/to/cookies.txt:/app/cookies.txt \
  -v /var/tmp/bunge-bits:/var/tmp/bunge-bits \
  ghcr.io/c12i/bunge-bits/stream-pulse:latest
```

Running the CLI via docker:

```bash
docker run --rm \
  -e OPENAI_API_KEY="..." \
  -e DATABASE_URL="..." \
  -e YTDLP_COOKIES_PATH=/app/cookies.txt \
  -v /path/to/cookies.txt:/app/cookies.txt \
  -v /var/tmp/bunge-bits:/var/tmp/bunge-bits \
  ghcr.io/c12i/bunge-bits/stream-pulse:latest \
  stream-pulse run --max-streams 2
```

### Explanation of Volume Mounts

| Mount                                        | Purpose                                                                                                                                                                                                                                                                                               |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `-v /path/to/cookies.txt:/app/cookies.txt`   | Provides the container with a valid YouTube `cookies.txt` file. This allows `yt-dlp` to access age-restricted or authenticated videos using a pre-exported session from a browser (see [yt-dlp cookies.txt setup](../ytdlp_bindings/README.md#using-cookiestxt-for-authenticated-youtube-downloads)). |
| `-v /var/tmp/bunge-bits:/var/tmp/bunge-bits` | Ensures that downloaded audio and intermediate files are persisted between runs. Without this, Docker would discard the files after each container restart.                                                                                                                                           |
