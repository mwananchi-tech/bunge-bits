//! # Yt Parser
//!
//! This module provides functionality to scrape and parse stream data from YouTube,
//! specifically tailored for the Parliament of Kenya Channel live streams.

use std::{ops::Deref, sync::LazyLock};

use regex::Regex;
use serde::de::DeserializeOwned;
use serde_json::Value;
use stream_datastore::Stream;

use crate::{error::Error, types::VideoRenderer};

static YT_INTIALDATA_RE: LazyLock<Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?s)<script[^>]*>\s*var\s+ytInitialData\s*=\s*(\{.*?\});\s*</script>")
        .unwrap()
});

/// Parses multiple streams from the provided JSON data.
///
/// # Parameters
/// * `json`: A reference to a `Value` containing the YouTube page's JSON data.
///
/// # Returns
/// * `Ok(Vec<Stream>)` containing all successfully parsed streams.
/// * `Err(YtScrapeError)` if the JSON structure is unexpected or parsing fails.
#[tracing::instrument(skip(json))]
pub fn parse_streams(json: &Value) -> Result<Vec<Stream>, Error> {
    let mut streams = Vec::new();

    if let Some(contents) = json["contents"]["twoColumnBrowseResultsRenderer"]["tabs"]
        .get(2)
        .ok_or(Error::ParseError("Failed to get item at idx 2 from ytInitialData['contents']['twoColumnBrowseResultsRenderer']['tabs']"))
        .map(|tab| tab["tabRenderer"]["content"]["richGridRenderer"]["contents"].as_array())?
    {
        for item in contents {
            if let Ok(video_renderer) =
                item["richItemRenderer"]["content"]["videoRenderer"].as_object()
                .ok_or(Error::ParseError("Failed to get item['richItemRenderer']['content']['videoRenderer']"))
            {
                let video_renderer =
                    serde_json::from_value::<VideoRenderer>(Value::Object(video_renderer.clone()))?;
                // Only process the video if it's not an upcoming / live event
                if video_renderer.upcoming_event_data.is_some() || video_renderer.view_count_text.is_none() || video_renderer.published_time_text.is_none() {
                    continue;
                }
                let stream = Stream::try_from(video_renderer)?;

                //XXX: Skip if duration is < 10 minutes
                if let Some(duration_secs) = parse_duration_to_seconds(&stream.duration) {
                    if duration_secs < 600 {
                        continue;
                    }
                } else {
                    // XXX: skip if duration could not be parsed
                    continue;
                }

                streams.push(stream);
            }
        }
    } else {
        return Err(Error::ParseError(
            "Failed to get script contents, structure might have changed",
        ));
    }

    Ok(streams)
}

fn parse_duration_to_seconds(duration_str: &str) -> Option<u64> {
    let parts: Vec<u64> = duration_str
        .split(':')
        .filter_map(|p| p.parse::<u64>().ok())
        .collect();

    match parts.len() {
        3 => Some(parts[0] * 3600 + parts[1] * 60 + parts[2]), // HH:MM:SS
        2 => Some(parts[0] * 60 + parts[1]),                   // MM:SS
        1 => Some(parts[0]),                                   // SS (rare)
        _ => None,
    }
}

impl TryFrom<VideoRenderer> for Stream {
    type Error = Error;

    /// Attempts to create a `Stream` from a videoRenderer object.
    ///
    /// # Parameters
    /// * `video_renderer`: A reference to a `Map<String, Value>` containing the video data.
    ///
    /// # Returns
    /// * `Ok(Stream)` if parsing is successful.
    /// * `Err(YtScrapeError)` if any required field is missing or cannot be parsed.
    fn try_from(
        VideoRenderer {
            video_id,
            title,
            published_time_text,
            view_count_text,
            length_text,
            ..
        }: VideoRenderer,
    ) -> Result<Self, Self::Error> {
        let title = &title
            .runs
            .first()
            .ok_or(Error::ParseError(
                "Failed to get video title via ['title']['runs'][0]['text']",
            ))?
            .text;
        let view_count = view_count_text
            .ok_or(Error::ParseError("No value found for 'viewCountText'"))
            .unwrap_or_default()
            .simple_text
            .ok_or(Error::ParseError("No valuefound for 'simpleText'"))
            .unwrap_or_default();
        let streamed_date = published_time_text
            .ok_or(Error::ParseError("No value found for 'publishedTimeText'"))
            .unwrap_or_default()
            .simple_text
            .ok_or(Error::ParseError("No value found for 'simpleText'"))
            .unwrap_or_default();
        let duration = length_text
            .ok_or(Error::ParseError("No value found for 'lengthText'"))?
            .simple_text;

        let stream = Stream {
            video_id,
            title: title.to_string(),
            view_count,
            streamed_date,
            duration,
            ..Default::default()
        };

        Ok(stream)
    }
}

pub struct YtHtmlDocument(String);

impl Deref for YtHtmlDocument {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl YtHtmlDocument {
    pub fn new(doc: String) -> Self {
        YtHtmlDocument(doc)
    }

    pub fn to_json<T>(&self) -> Result<T, crate::error::Error>
    where
        T: DeserializeOwned,
    {
        let result = YT_INTIALDATA_RE
            .captures(self)
            .and_then(|cap| cap.get(1))
            .and_then(|m| serde_json::from_str(m.as_str()).ok())
            .ok_or(Error::ParseError(
                "Failed to extract ytInitialData from the page's script tag",
            ));

        result
    }
}

impl From<String> for YtHtmlDocument {
    fn from(value: String) -> Self {
        YtHtmlDocument(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_successful_extraction() {
        let html = r#"
            <html>
                <head>
                    <script nonce="gZTn8MILMQFuWon1rDk2VA">
                        var ytInitialData = {"key": "value", "number": 42};
                    </script>
                </head>
                <body>
                    <p>Some content</p>
                </body>
            </html>
        "#;

        let doc = YtHtmlDocument::from(html.to_string());
        let result = doc.to_json::<Value>();
        assert!(result.is_ok(), "Failed to extract JSON: {:?}", result.err());
        assert_eq!(result.unwrap(), json!({"key": "value", "number": 42}));
    }

    #[test]
    fn test_extraction_with_special_characters() {
        let html = r#"
            <script nonce="gZTn8MILMQFuWon1rDk2VA">
                var ytInitialData = {
                    "key": "value with \"quotes\" and \n newline"
                };
            </script>
        "#;

        let doc = YtHtmlDocument::from(html.to_string());
        let json = doc
            .to_json::<Value>()
            .expect("Failed to extract JSON with special characters");
        assert_eq!(
            json["key"].as_str().unwrap().trim(),
            "value with \"quotes\" and \n newline"
        );
    }

    #[test]
    fn test_extraction_with_multiple_occurrences() {
        let html = r#"
            <script nonce="gZTn8MILMQFuWon1rDk2VA">var ytInitialData = {"first": true};</script>
            <script nonce="gZTn8MILMQFuWon1rDk2VA">
                var ytInitialData = {"second": true};
            </script>
        "#;

        let doc = YtHtmlDocument::from(html.to_string());
        let json = doc
            .to_json::<Value>()
            .expect("Failed to extract first JSON");
        assert_eq!(json, json!({"first": true}));
    }

    #[test]
    fn test_extraction_with_no_data() {
        let html = r#"
            <html>
                <body>
                    <p>No ytInitialData here</p>
                </body>
            </html>
        "#;

        let doc = YtHtmlDocument::from(html.to_string());
        let result = doc.to_json::<Value>();
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::ParseError(_))));
    }

    #[test]
    fn test_extraction_with_invalid_json() {
        let html = r#"
            <script nonce="gZTn8MILMQFuWon1rDk2VA">
                var ytInitialData = {invalid: json};
            </script>
        "#;

        let doc = YtHtmlDocument::from(html.to_string());
        let result = doc.to_json::<Value>();
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::ParseError(_))));
    }

    #[test]
    fn test_fixture_parses_streams() {
        let html = include_str!("../../tests/fixtures/yt.html");
        let doc = YtHtmlDocument::new(html.to_string());

        let json = doc
            .to_json::<Value>()
            .expect("Failed to extract ytInitialData");

        let streams = parse_streams(&json).expect("Failed to parse streams");

        assert!(
            !streams.is_empty(),
            "Fixture should contain parseable streams"
        );
        assert!(
            streams.len() >= 10,
            "Expected at least 10 streams, got {}",
            streams.len()
        );

        for stream in &streams {
            assert!(!stream.video_id.is_empty(), "video_id should not be empty");
            assert!(!stream.title.is_empty(), "title should not be empty");
            assert!(
                !stream.duration.is_empty(),
                "duration should not be empty for {}",
                stream.video_id
            );
            assert!(
                !stream.streamed_date.is_empty(),
                "streamed_date should not be empty for {}",
                stream.video_id
            );

            // duration should be >= 10 minutes (parser filters < 600s)
            let parts: Vec<u64> = stream
                .duration
                .split(':')
                .filter_map(|p| p.parse().ok())
                .collect();
            let secs = match parts.len() {
                3 => parts[0] * 3600 + parts[1] * 60 + parts[2],
                2 => parts[0] * 60 + parts[1],
                _ => 0,
            };
            assert!(
                secs >= 600,
                "Stream {} duration {}s should be >= 600s",
                stream.video_id,
                secs
            );
        }
    }
}
