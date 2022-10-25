use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::Deserialize;

use super::RefreshToken;

const TOKEN_URI: &str = "https://oauth2.googleapis.com/token";

#[derive(Debug, Deserialize)]
pub struct Config {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
    pub client_id: String,
    pub client_secret: String,
}

impl Config {
    pub fn is_valid(&self) -> Result<bool> {
        let now = UNIX_EPOCH
            .elapsed()
            .with_context(|| "Time went backwards!")?;

        let exp = Duration::from_secs(self.expires_at);

        Ok(now < exp)
    }

    pub async fn refresh(&mut self) -> Result<()> {
        let token = crate::google_drive::HTTP
            .post(TOKEN_URI)
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", self.refresh_token.as_str()),
            ])
            .send()
            .await
            .with_context(|| format!("Could not send post request to '{}'", TOKEN_URI))?
            .json::<RefreshToken>()
            .await?;

        let expires_at = UNIX_EPOCH
            .elapsed()
            .with_context(|| "Time went backwards!")?
            + Duration::from_secs(token.expires_in);

        self.expires_at = expires_at.as_secs();
        self.access_token = token.access_token;

        Ok(())
    }
}
