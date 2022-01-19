use std::path;

use serde::{Deserialize, Serialize};

mod local;

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMeta {
    name: String,
    file_type: FileType,
    size: u64,
    id: FileId,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FileType {
    File,
    Dir,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileId(FileSource, String);

#[derive(Debug, Serialize, Deserialize)]
pub enum FileSource {
    Local,
}

pub async fn list_meta(id: &FileId) -> anyhow::Result<Vec<FileMeta>> {
    let FileId(source, id) = id;

    match source {
        FileSource::Local => local::list_meta(path::Path::new(id)).await,
    }
}
