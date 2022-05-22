use anyhow::Result;
use reqwest::Client;

#[derive(Debug, Clone)]
pub struct BlindsController {
    http_client: Client,
    url: String,
}

impl BlindsController {
    pub fn new(url: String) -> Self {
        let http_client = Client::new();
        Self { http_client, url }
    }

    pub async fn open_blinds(&self) -> Result<()> {
        let url = format!("{}/open_blinds", self.url);
        self.http_client
            .post(&url)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn close_blinds(&self) -> Result<()> {
        let url = format!("{}/close_blinds", self.url);
        self.http_client
            .post(&url)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}
