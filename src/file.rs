use serde::{Deserialize, Serialize};
use std::path;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::io as uio;

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

async fn get_meta(id: &FileId) -> anyhow::Result<FileMeta> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::get_meta(path::Path::new(id)).await,
    }
}

pub async fn list_meta(id: &FileId) -> anyhow::Result<Vec<FileMeta>> {
    let FileId(source, id) = id;

    match source {
        FileSource::Local => local::list_meta(path::Path::new(id)).await,
    }
}

async fn read(id: &FileId) -> anyhow::Result<uio::ReaderStream<impl AsyncRead>> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::read(path::Path::new(id)).await,
    }
}

async fn write(id: &FileId) -> anyhow::Result<impl AsyncWrite> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::write(path::Path::new(id)).await,
    }
}

pub async fn create_file(name: &str, dir: &FileId) -> anyhow::Result<FileId> {
    let FileId(source, id) = dir;
    match source {
        FileSource::Local => local::create_file(name, path::Path::new(id)).await,
    }
}

pub async fn create_dir(name: &str, dir: &FileId) -> anyhow::Result<FileId> {
    let FileId(source, id) = dir;
    match source {
        FileSource::Local => local::create_dir(name, path::Path::new(id)).await,
    }
}
