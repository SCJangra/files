mod config;
mod drive_file;
mod upload;

pub use config::Config;
pub use drive_file::DriveFile;
pub use upload::Upload;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RefreshToken {
    pub access_token: String,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResponse {
    pub next_page_token: Option<String>,
    pub files: Vec<DriveFile>,
}
