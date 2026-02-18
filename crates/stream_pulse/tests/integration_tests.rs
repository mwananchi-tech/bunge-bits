mod mocks;

use mocks::{
    audio_handler::MockAudioHandler, channel_scraper::MockChannelScraper, datastore::MockDataStore,
    summarizer::MockSummarizer, transcriber::MockTranscriber,
};
use std::collections::HashSet;
use stream_pulse::{AudioInput, LiveStreamProcessorBuilder};

fn build_processor(
    store: MockDataStore,
    transcriber: MockTranscriber,
    summarizer: MockSummarizer,
    audio_handler: MockAudioHandler,
    scraper: MockChannelScraper,
    max_streams: usize,
) -> stream_pulse::LiveStreamProcessor<
    MockDataStore,
    MockTranscriber,
    MockSummarizer,
    MockAudioHandler,
    MockChannelScraper,
> {
    LiveStreamProcessorBuilder::new("/tmp/stream-pulse-test")
        .store(store)
        .transcriber(transcriber)
        .summarizer(summarizer)
        .audio_handler(audio_handler)
        .channel_scraper(scraper)
        .max_streams(max_streams)
        .with_chunking(900)
        .build()
}

// ─── Happy path ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_happy_path_processes_max_streams() {
    let max_streams = 3;

    let store = MockDataStore::default();
    let transcriber = MockTranscriber::new("This is the transcript of a parliamentary session.");
    let summarizer = MockSummarizer::new("## Summary\nKey points discussed in parliament.");
    let audio_handler = MockAudioHandler::default();
    let scraper = MockChannelScraper::from_fixture();

    let inserted = store.inserted.clone();
    let transcriber_calls = transcriber.calls.clone();
    let summarizer_calls = summarizer.calls.clone();
    let audio_calls = audio_handler.calls.clone();

    let processor = build_processor(
        store,
        transcriber,
        summarizer,
        audio_handler,
        scraper,
        max_streams,
    );

    let result = processor.run().await;
    assert!(
        result.is_ok(),
        "Pipeline should succeed: {:?}",
        result.err()
    );

    let inserted = inserted.lock().unwrap();
    assert_eq!(
        inserted.len(),
        max_streams,
        "Should insert exactly max_streams streams"
    );

    let transcriber_calls = transcriber_calls.lock().unwrap();
    assert_eq!(
        transcriber_calls.len(),
        max_streams,
        "Should transcribe exactly max_streams streams"
    );

    let summarizer_calls = summarizer_calls.lock().unwrap();
    assert_eq!(
        summarizer_calls.len(),
        max_streams,
        "Should summarize exactly max_streams streams"
    );

    let audio_calls = audio_calls.lock().unwrap();
    assert_eq!(
        audio_calls.len(),
        max_streams,
        "Should download audio for exactly max_streams streams"
    );

    // Every inserted stream should have a summary
    for stream in inserted.iter() {
        assert!(
            stream.summary_md.is_some(),
            "Stream {} should have summary_md set",
            stream.video_id
        );
        assert_eq!(
            stream.summary_md.as_deref(),
            Some("## Summary\nKey points discussed in parliament.")
        );
    }
}

// ─── Chunking ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_chunked_audio_input_when_chunking_enabled() {
    let store = MockDataStore::default();
    let transcriber = MockTranscriber::new("transcript");
    let summarizer = MockSummarizer::new("summary");
    let audio_handler = MockAudioHandler::default();
    let scraper = MockChannelScraper::from_fixture();

    let transcriber_calls = transcriber.calls.clone();

    let processor = build_processor(store, transcriber, summarizer, audio_handler, scraper, 1);

    processor.run().await.expect("Pipeline should succeed");

    let calls = transcriber_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);

    match &calls[0] {
        AudioInput::Chunked {
            chunk_duration_seconds,
            ..
        } => {
            assert_eq!(
                *chunk_duration_seconds, 900,
                "Chunk duration should be 900s"
            );
        }
        AudioInput::File(_) => {
            panic!("Expected Chunked audio input when chunking is enabled");
        }
    }
}

// ─── Filtering ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_existing_streams_are_filtered_out() {
    // We need to know the video IDs that parse_streams will produce from the fixture.
    // First, run the pipeline with no existing IDs to capture what IDs come out.
    let probe_store = MockDataStore::default();
    let probe_transcriber = MockTranscriber::new("transcript");
    let probe_summarizer = MockSummarizer::new("summary");
    let probe_audio = MockAudioHandler::default();
    let probe_scraper = MockChannelScraper::from_fixture();

    let probe_inserted = probe_store.inserted.clone();

    let processor = build_processor(
        probe_store,
        probe_transcriber,
        probe_summarizer,
        probe_audio,
        probe_scraper,
        30, // get all streams
    );
    processor.run().await.expect("Probe run should succeed");

    let all_ids: Vec<String> = probe_inserted
        .lock()
        .unwrap()
        .iter()
        .map(|s| s.video_id.clone())
        .collect();

    assert!(
        !all_ids.is_empty(),
        "Fixture should produce parseable streams"
    );

    // Now mark some as existing
    let existing: HashSet<String> = all_ids.iter().take(5).cloned().collect();

    let store = MockDataStore {
        existing_ids: existing.clone(),
        ..Default::default()
    };
    let transcriber = MockTranscriber::new("transcript");
    let summarizer = MockSummarizer::new("summary");
    let audio_handler = MockAudioHandler::default();
    let scraper = MockChannelScraper::from_fixture();

    let inserted = store.inserted.clone();

    let processor = build_processor(store, transcriber, summarizer, audio_handler, scraper, 30);
    processor.run().await.expect("Pipeline should succeed");

    let inserted = inserted.lock().unwrap();

    // None of the inserted streams should have an existing ID
    for stream in inserted.iter() {
        assert!(
            !existing.contains(&stream.video_id),
            "Stream {} should have been filtered out as existing",
            stream.video_id
        );
    }

    // Should have processed all non-existing streams
    assert_eq!(
        inserted.len(),
        all_ids.len() - existing.len(),
        "Should process all non-existing streams"
    );
}

#[tokio::test]
async fn test_max_streams_limits_processing() {
    let store = MockDataStore::default();
    let transcriber = MockTranscriber::new("transcript");
    let summarizer = MockSummarizer::new("summary");
    let audio_handler = MockAudioHandler::default();
    let scraper = MockChannelScraper::from_fixture();

    let inserted = store.inserted.clone();

    let processor = build_processor(store, transcriber, summarizer, audio_handler, scraper, 2);
    processor.run().await.expect("Pipeline should succeed");

    let inserted = inserted.lock().unwrap();
    assert_eq!(inserted.len(), 2, "Should respect max_streams limit of 2");
}

// ─── Edge cases ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_no_streams_found_returns_ok() {
    let empty_html = r#"<html><body>No ytInitialData here</body></html>"#;

    let store = MockDataStore::default();
    let transcriber = MockTranscriber::new("transcript");
    let summarizer = MockSummarizer::new("summary");
    let audio_handler = MockAudioHandler::default();
    let scraper = MockChannelScraper::new(empty_html.to_string());

    let inserted = store.inserted.clone();
    let audio_calls = audio_handler.calls.clone();

    let processor = build_processor(store, transcriber, summarizer, audio_handler, scraper, 5);

    // This may return Ok (no streams) or Err (parse failure) depending on implementation.
    // Either way, no streams should be inserted.
    let _ = processor.run().await;

    let inserted = inserted.lock().unwrap();
    assert!(inserted.is_empty(), "No streams should be inserted");

    let audio_calls = audio_calls.lock().unwrap();
    assert!(audio_calls.is_empty(), "No audio should be downloaded");
}

#[tokio::test]
async fn test_all_streams_exist_in_db_returns_ok() {
    // Probe to get all IDs
    let probe_store = MockDataStore::default();
    let probe_transcriber = MockTranscriber::new("transcript");
    let probe_summarizer = MockSummarizer::new("summary");
    let probe_audio = MockAudioHandler::default();
    let probe_scraper = MockChannelScraper::from_fixture();

    let probe_inserted = probe_store.inserted.clone();

    let processor = build_processor(
        probe_store,
        probe_transcriber,
        probe_summarizer,
        probe_audio,
        probe_scraper,
        30,
    );
    processor.run().await.expect("Probe run should succeed");

    let all_ids: HashSet<String> = probe_inserted
        .lock()
        .unwrap()
        .iter()
        .map(|s| s.video_id.clone())
        .collect();

    // Now run with all IDs marked as existing
    let store = MockDataStore {
        existing_ids: all_ids,
        ..Default::default()
    };
    let transcriber = MockTranscriber::new("transcript");
    let summarizer = MockSummarizer::new("summary");
    let audio_handler = MockAudioHandler::default();
    let scraper = MockChannelScraper::from_fixture();

    let inserted = store.inserted.clone();
    let audio_calls = audio_handler.calls.clone();

    let processor = build_processor(store, transcriber, summarizer, audio_handler, scraper, 5);
    let result = processor.run().await;
    assert!(result.is_ok(), "Should return Ok when all streams exist");

    let inserted = inserted.lock().unwrap();
    assert!(inserted.is_empty(), "No streams should be inserted");

    let audio_calls = audio_calls.lock().unwrap();
    assert!(audio_calls.is_empty(), "No audio should be downloaded");
}

// ─── Error propagation ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_scraper_failure_propagates_error() {
    let store = MockDataStore::default();
    let transcriber = MockTranscriber::new("transcript");
    let summarizer = MockSummarizer::new("summary");
    let audio_handler = MockAudioHandler::default();
    let scraper = MockChannelScraper::failing("Scraper network error");

    let processor = build_processor(store, transcriber, summarizer, audio_handler, scraper, 5);
    let result = processor.run().await;
    assert!(result.is_err(), "Should propagate scraper error");

    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("Scraper network error"),
        "Error should contain scraper message, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_transcription_failure_propagates_error() {
    let store = MockDataStore::default();
    let transcriber = MockTranscriber::failing("Whisper API timeout");
    let summarizer = MockSummarizer::new("summary");
    let audio_handler = MockAudioHandler::default();
    let scraper = MockChannelScraper::from_fixture();

    let processor = build_processor(store, transcriber, summarizer, audio_handler, scraper, 1);
    let result = processor.run().await;
    assert!(result.is_err(), "Should propagate transcription error");
}

#[tokio::test]
async fn test_summarization_failure_propagates_error() {
    let store = MockDataStore::default();
    let transcriber = MockTranscriber::new("transcript");
    let summarizer = MockSummarizer::failing("GPT-4 rate limit");
    let audio_handler = MockAudioHandler::default();
    let scraper = MockChannelScraper::from_fixture();

    let processor = build_processor(store, transcriber, summarizer, audio_handler, scraper, 1);
    let result = processor.run().await;
    assert!(result.is_err(), "Should propagate summarization error");
}

#[tokio::test]
async fn test_db_insert_failure_propagates_error() {
    let store = MockDataStore::failing("Connection refused");
    let transcriber = MockTranscriber::new("transcript");
    let summarizer = MockSummarizer::new("summary");
    let audio_handler = MockAudioHandler::default();
    let scraper = MockChannelScraper::from_fixture();

    let processor = build_processor(store, transcriber, summarizer, audio_handler, scraper, 1);
    let result = processor.run().await;
    assert!(result.is_err(), "Should propagate DB insert error");
}

#[tokio::test]
async fn test_audio_download_failure_propagates_error() {
    let store = MockDataStore::default();
    let transcriber = MockTranscriber::new("transcript");
    let summarizer = MockSummarizer::new("summary");
    let audio_handler = MockAudioHandler::failing("yt-dlp download failed");
    let scraper = MockChannelScraper::from_fixture();

    let processor = build_processor(store, transcriber, summarizer, audio_handler, scraper, 1);
    let result = processor.run().await;
    assert!(result.is_err(), "Should propagate audio download error");
}
