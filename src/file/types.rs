#[derive(Debug, Clone)]
pub struct FileMeta {
    pub name: String,
    pub file_type: FileType,
    pub size: u64,
    pub id: FileId,
    pub parent_id: Option<FileId>,
}

#[derive(Debug, Clone)]
pub enum FileType {
    File,
    Dir,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct FileId(pub FileSource, pub String);

#[derive(Debug, Clone)]
pub enum FileSource {
    Local,
}

#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub total: u64,
    pub done: u64,
    pub percent: f64,
}
