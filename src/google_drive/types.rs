use std::time::{Duration, UNIX_EPOCH};

use crate::types::*;
use anyhow::Context;
use fievar::Fields;
use serde::Deserialize;

const FOLDER: &str = "application/vnd.google-apps.folder";
const TOKEN_URI: &str = "https://oauth2.googleapis.com/token";

#[derive(Debug, Deserialize)]
pub struct RefreshToken {
    pub access_token: String,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
    pub client_id: String,
    pub client_secret: String,
}

impl Config {
    pub fn is_valid(&self) -> anyhow::Result<bool> {
        let now = UNIX_EPOCH
            .elapsed()
            .with_context(|| "Time went backwards!")?;

        let exp = Duration::from_secs(self.expires_at);

        Ok(now < exp)
    }

    pub async fn refresh(&mut self) -> anyhow::Result<()> {
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

#[derive(Debug, Deserialize, Fields)]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    #[serde(rename = "mimeType")]
    #[fievar(name = "mimeType")]
    pub mime_type: String,
    pub size: Option<String>,
    pub parents: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResponse {
    pub next_page_token: Option<String>,
    pub files: Vec<DriveFile>,
}

impl From<(DriveFile, &str)> for FileMeta {
    fn from((file, source_name): (DriveFile, &str)) -> Self {
        let file_source = FileSource::GoogleDrive(source_name.to_string());

        let size: u64 = file.size.map_or(0, |s| s.parse::<u64>().unwrap());

        let parent_id = file
            .parents
            .map(|mut p| FileId(file_source.clone(), p.swap_remove(0)));

        let file_type = match file.mime_type.as_str() {
            FOLDER => FileType::Dir,
            _ => FileType::File,
        };

        let id = FileId(file_source, file.id);

        Self {
            name: file.name,
            file_type,
            id,
            size,
            parent_id,
        }
    }
}
