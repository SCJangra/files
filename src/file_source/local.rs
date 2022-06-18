use crate::types::*;

use anyhow::Context;
use async_stream::stream;
use futures::{Stream, TryStreamExt};
use std::path;
use tokio::{
    fs,
    io::{AsyncRead, AsyncWrite},
    task,
};
use tokio_stream::wrappers as tsw;
use unwrap_or::unwrap_ok_or;

pub async fn get_meta(path: &path::Path) -> anyhow::Result<FileMeta> {
    let id = path.to_string_lossy().to_string();
    let parent_id = path
        .parent()
        .map(|p| FileId(FileSource::Local, p.to_string_lossy().to_string()));

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
        parent_id,
    })
}

pub fn list_meta(path: &path::Path) -> impl Stream<Item = anyhow::Result<FileMeta>> + '_ {
    stream! {
        let id = path.to_string_lossy().to_string();
        let rd = fs::read_dir(path)
            .await
            .with_context(|| format!("Could not read directory '{}'", id));
        let rd = unwrap_ok_or!(rd, e, {
            yield Err(e);
            return;
        });

        let s = tsw::ReadDirStream::new(rd)
            .map_err(move |e| {
                anyhow::Error::new(e).context(format!("Error while reading directory '{}'", id))
            })
            .and_then(|d| async move { get_meta(d.path().as_path()).await });
        for await v in s { yield v; }
    }
}

pub async fn read(path: &path::Path) -> anyhow::Result<impl AsyncRead> {
    let file = fs::File::open(path)
        .await
        .with_context(|| format!("Could not read file '{}'", path.to_string_lossy()))?;

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

    let file =
        file.with_context(|| format!("Could not write to file '{}'", path.to_string_lossy()))?;

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

pub async fn rename(file: &path::Path, new_name: &str) -> anyhow::Result<FileId> {
    let mut path = path::PathBuf::from(file);
    path.set_file_name(new_name);

    if path.exists() {
        return Err(anyhow::anyhow!(
            "A file with name '{}' already exists!",
            new_name.to_string()
        ));
    }

    fs::rename(file, path.as_path())
        .await
        .with_context(|| format!("Could not rename file '{}'", file.to_string_lossy()))?;

    let id = FileId(FileSource::Local, path.to_string_lossy().to_string());

    Ok(id)
}

pub async fn mv(file: &path::Path, dir: &path::Path) -> anyhow::Result<FileId> {
    let mut dir_pb = path::PathBuf::from(dir);
    let name = match file.file_name() {
        Some(n) => n,
        None => {
            return Err(anyhow::anyhow!(
                "Moving files without names is currently not supported!"
            ));
        }
    };
    dir_pb.push(name);

    let p = dir_pb.to_string_lossy().to_string();

    fs::rename(file, dir_pb.as_path()).await.with_context(|| {
        format!(
            "Could not move file '{}' to '{}'",
            file.to_string_lossy(),
            p.clone(),
        )
    })?;

    let id = FileId(FileSource::Local, p);
    Ok(id)
}

pub async fn delete_file(file: &path::Path) -> anyhow::Result<bool> {
    fs::remove_file(file)
        .await
        .with_context(|| format!("Could not delete file '{}'", file.to_string_lossy()))?;
    Ok(true)
}

pub async fn delete_dir(dir: &path::Path) -> anyhow::Result<bool> {
    fs::remove_dir(dir)
        .await
        .with_context(|| format!("Could not delete directory '{}'", dir.to_string_lossy()))?;
    Ok(true)
}

pub async fn get_mime(file: &path::Path) -> anyhow::Result<String> {
    let file = file.to_owned();

    task::spawn_blocking(move || {
        tree_magic_mini::from_filepath(file.as_path())
            .map(|s| s.to_string())
            .with_context(|| {
                format!(
                    "Could not get mime type for file '{}'",
                    file.to_string_lossy()
                )
            })
    })
    .await?
}
