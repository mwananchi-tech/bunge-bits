use std::ops::Deref;

use crate::yt::ChannelScraper;

pub struct Scraper(pub reqwest::Client);

impl Deref for Scraper {
    type Target = reqwest::Client;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ChannelScraper for Scraper {
    const CHANNEL_URL: &str = "https://www.youtube.com/@ParliamentofKenyaChannel/streams";

    type Error = anyhow::Error;

    async fn scrape_channel(&self) -> Result<crate::parser::YtHtmlDocument, Self::Error> {
        let yt_html_document = self
            .get(Self::CHANNEL_URL)
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await?
            .text()
            .await?;

        Ok(yt_html_document.into())
    }
}
