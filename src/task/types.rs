use serde::{Deserialize, Serialize};

use crate::file;

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TaskResult {
    List(Vec<file::FileMeta>),
    Create(file::FileId),
    CopyFileProgress(CopyFileProgress),
}

pub enum Task {
    List(file::FileId),
    Create {
        name: String,
        dir: file::FileId,
    },
    CopyFile {
        source: file::FileId,
        dest: file::FileId,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CopyFileProgress {
    pub total: u64,
    pub done: u64,
    pub percent: f64,
}
