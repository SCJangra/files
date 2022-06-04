use super::*;
use fievar::Fields;
use serde::Deserialize;

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
    pub size: Option<u64>,
    pub parents: Option<Vec<String>>,
}

impl From<(DriveFile, &str)> for FileMeta {
    fn from((file, source_name): (DriveFile, &str)) -> Self {
        let file_source = FileSource::GoogleDrive(source_name.to_string());

        let size = file.size.unwrap_or(0);

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
