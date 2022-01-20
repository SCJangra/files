use serde::{Deserialize, Serialize};

use crate::file;

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TaskResult {
    ListResult(Vec<file::FileMeta>),
    CreateResult(file::FileId),
}

pub enum Task {
    List(file::FileId),
    Create { name: String, dir: file::FileId },
}
