use crate::types::google_drive::*;
use anyhow::Context;
use reqwest::Client;
use std::{
    collections::HashMap,
    time::{Duration, UNIX_EPOCH},
};
use tokio::sync::RwLock;

const TOKEN_URI: &str = "https://oauth2.googleapis.com/token";

lazy_static::lazy_static! {
    pub static ref CONFIGS: RwLock<HashMap<String, Config>> = RwLock::new(HashMap::<String, Config>::new());
    pub static ref HTTP: Client = Client::new();
}

async fn refresh_token(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> anyhow::Result<RefreshToken> {
    let t = HTTP
        .post(TOKEN_URI)
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .with_context(|| format!("Could not send post request to '{}'", TOKEN_URI))?
        .json::<RefreshToken>()
        .await?;

    Ok(t)
}

pub async fn get_tokne(name: &str) -> anyhow::Result<String> {
    let c = CONFIGS.read().await;
    let config = c
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("A config with name {} does not exist", name))?;

    let now = UNIX_EPOCH
        .elapsed()
        .with_context(|| "Time went backwards!")?;
    let exp = Duration::from_secs(config.expires_at);

    if now < exp {
        return Ok(config.access_token.to_string());
    }

    let token = refresh_token(
        &config.client_id,
        &config.client_secret,
        &config.refresh_token,
    )
    .await?;

    let expires_at = UNIX_EPOCH
        .elapsed()
        .with_context(|| "Time went backwards!")?
        + Duration::from_secs(token.expires_in);

    {
        // dropping this read guard is necessary to obtain a write guard
        drop(c);
        let mut c = CONFIGS.write().await;
        let mut config = c.get_mut(name).unwrap();

        config.expires_at = expires_at.as_secs();
        config.access_token = token.access_token.clone();
    }

    Ok(token.access_token)
}
