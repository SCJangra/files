use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMeta {
    pub name: String,
    pub file_type: FileType,
    pub size: u64,
    pub id: FileId,
    pub parent_id: Option<FileId>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Progress {
    pub total: u64,
    pub done: u64,
    pub percent: f64,
}
