use futures as futs;
use std::path;
use tokio::io::{AsyncRead, AsyncWrite};
pub use types::*;

mod local;
mod types;

pub async fn get_meta(id: &FileId) -> anyhow::Result<FileMeta> {
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

pub async fn read(id: &FileId) -> anyhow::Result<impl AsyncRead> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::read(path::Path::new(id)).await,
    }
}

pub async fn write(id: &FileId, overwrite: bool) -> anyhow::Result<impl AsyncWrite> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::write(path::Path::new(id), overwrite).await,
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

pub async fn copy_file(
    source: &FileId,
    dest: &FileId,
) -> anyhow::Result<(impl AsyncRead, impl AsyncWrite)> {
    let fm = get_meta(source).await?;
    let dest = create_file(&fm.name, dest).await?;

    let rw = futs::try_join!(read(source), write(&dest, true))?;
    Ok(rw)
}

pub async fn rename(id: &FileId, new_name: &str) -> anyhow::Result<FileId> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::rename(path::Path::new(id), new_name).await,
    }
}
