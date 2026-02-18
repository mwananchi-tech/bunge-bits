use stream_pulse::{parser::YtHtmlDocument, yt::ChannelScraper};

#[derive(Clone)]
pub struct MockChannelScraper {
    pub html: String,
    pub fail_with: Option<String>,
}

impl MockChannelScraper {
    pub fn new(html: String) -> Self {
        Self {
            html,
            fail_with: None,
        }
    }

    pub fn from_fixture() -> Self {
        Self::new(include_str!("../fixtures/yt.html").to_string())
    }

    pub fn failing(msg: &str) -> Self {
        Self {
            html: String::new(),
            fail_with: Some(msg.to_string()),
        }
    }
}

impl ChannelScraper for MockChannelScraper {
    const CHANNEL_URL: &'static str = "https://youtube.com/mock";
    type Error = anyhow::Error;

    async fn scrape_channel(&self) -> anyhow::Result<YtHtmlDocument> {
        if let Some(ref msg) = self.fail_with {
            return Err(anyhow::anyhow!("{}", msg));
        }
        Ok(YtHtmlDocument::new(self.html.clone()))
    }
}
