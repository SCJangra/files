use super::*;
use anyhow::Context;
use fievar::Fields;
use reqwest::{Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize};

const FOLDER: &str = "application/vnd.google-apps.folder";

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

pub struct Res(Response);

impl Res {
    pub async fn json<T: DeserializeOwned>(self) -> anyhow::Result<T> {
        let res = self.0;
        let status = res.status();

        if status != StatusCode::OK {
            let text = res.text().await.with_context(|| "Could not get response")?;
            return Err(anyhow::anyhow!("GoogleAPIError {} {}", status, text));
        }

        let bytes = res
            .bytes()
            .await
            .with_context(|| "Could not get response")?;
        let t = serde_json::from_slice::<T>(&bytes)?;
        Ok(t)
    }
}

impl From<Response> for Res {
    fn from(r: Response) -> Self {
        Self(r)
    }
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
