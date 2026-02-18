FROM rust:1.88 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin stream-pulse

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
  ca-certificates \
  curl \
  xz-utils \
  python3 \
  && rm -rf /var/lib/apt/lists/*

RUN curl -L https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz \
  | tar xJ --strip-components=1 -C /usr/local/bin --wildcards '*/ffmpeg' '*/ffprobe'

COPY --from=builder /app/target/release/stream-pulse /usr/local/bin/stream-pulse

WORKDIR /app

RUN mkdir -p /var/tmp/bunge-bits

ENV OPENAI_API_KEY= \
  DATABASE_URL= \
  SENTRY_DSN= \
  MAX_STREAMS_TO_PROCESS= \
  YTDLP_COOKIES_PATH= \
  CRON_SCHEDULE=

CMD ["stream-pulse", "cron"]
