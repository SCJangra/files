use fievar::Fields;
use serde::Deserialize;

use crate::*;

const FOLDER: &str = "application/vnd.google-apps.folder";

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

impl From<(DriveFile, &str)> for File {
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
