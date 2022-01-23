use super::{FileId, FileMeta, FileSource, FileType};
use anyhow::Context;
use futures::TryStreamExt;
use std::path;
use tokio::{
    fs,
    io::{AsyncRead, AsyncWrite},
};
use tokio_stream::wrappers as tsw;

pub async fn get_meta(path: &path::Path) -> anyhow::Result<FileMeta> {
    let id = path.to_string_lossy().to_string();

    let name = match path.file_name() {
        Some(n) => n.to_string_lossy().to_string(),
        None => String::from("None"),
    };

    let meta = fs::metadata(path)
        .await
        .with_context(|| format!("Could not get metadata for file '{}'", id))?;

    let size = meta.len();

    let file_type = if meta.is_file() {
        FileType::File
    } else if meta.is_dir() {
        FileType::Dir
    } else {
        FileType::Unknown
    };

    let id = FileId(FileSource::Local, id);

    Ok(FileMeta {
        name,
        id,
        file_type,
        size,
    })
}

pub async fn list_meta(path: &path::Path) -> anyhow::Result<Vec<FileMeta>> {
    let id = path.to_string_lossy().to_string();
    let rd = fs::read_dir(path)
        .await
        .with_context(|| format!("Could not read directory '{}'", id))?;

    let s = tsw::ReadDirStream::new(rd);

    let files = s
        .map_err(|e| {
            anyhow::Error::new(e).context(format!("Error while reading directory '{}'", id))
        })
        .and_then(|d| async move { get_meta(d.path().as_path()).await })
        .try_collect::<Vec<FileMeta>>()
        .await?;

    Ok(files)
}

pub async fn read(path: &path::Path) -> anyhow::Result<impl AsyncRead> {
    let file = fs::File::open(path).await.with_context(|| {
        format!(
            "Could not read file '{}'",
            path.to_string_lossy().to_string()
        )
    })?;

    Ok(file)
}

pub async fn write(path: &path::Path, overwrite: bool) -> anyhow::Result<impl AsyncWrite> {
    let file = if overwrite {
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .await
    } else {
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .await
    };

    let file = file.with_context(|| {
        format!(
            "Could not write to file '{}'",
            path.to_string_lossy().to_string()
        )
    })?;

    Ok(file)
}

pub async fn create_file(name: &str, dir: &path::Path) -> anyhow::Result<FileId> {
    let mut path = dir.to_path_buf();
    path.push(name);

    write(path.as_path(), false).await?;

    let id = FileId(FileSource::Local, path.to_string_lossy().to_string());

    Ok(id)
}

pub async fn create_dir(name: &str, dir: &path::Path) -> anyhow::Result<FileId> {
    let mut path = dir.to_path_buf();
    path.push(name);

    let id = path.to_string_lossy().to_string();

    fs::create_dir(path)
        .await
        .with_context(|| format!("Could not create directory '{}'", id))?;

    let id = FileId(FileSource::Local, id);

    Ok(id)
}
