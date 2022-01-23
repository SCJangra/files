use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMeta {
    pub name: String,
    pub file_type: FileType,
    pub size: u64,
    pub id: FileId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileType {
    File,
    Dir,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileId(pub FileSource, pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileSource {
    Local,
}
