use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMeta {
    pub name: String,
    pub file_type: FileType,
    pub size: u64,
    pub id: FileId,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FileType {
    File,
    Dir,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileId(pub FileSource, pub String);

#[derive(Debug, Serialize, Deserialize)]
pub enum FileSource {
    Local,
}
