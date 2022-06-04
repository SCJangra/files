use futures::Stream;
use std::path;
use tokio::io::{AsyncRead, AsyncWrite};

use super::file_source::local;

use super::types::*;

pub async fn get_meta(id: &FileId) -> anyhow::Result<FileMeta> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::get_meta(path::Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(_) => unimplemented!(),
    }
}

pub async fn list_meta(
    id: &FileId,
) -> anyhow::Result<impl Stream<Item = anyhow::Result<FileMeta>>> {
    let FileId(source, id) = id;

    match source {
        FileSource::Local => local::list_meta(path::Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(_) => unimplemented!(),
    }
}

pub async fn read(id: &FileId) -> anyhow::Result<impl AsyncRead> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::read(path::Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(_) => unimplemented!(),
    }
}

pub async fn write(id: &FileId, overwrite: bool) -> anyhow::Result<impl AsyncWrite> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::write(path::Path::new(id), overwrite).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(_) => unimplemented!(),
    }
}

pub async fn create_file(name: &str, dir: &FileId) -> anyhow::Result<FileId> {
    let FileId(source, id) = dir;
    match source {
        FileSource::Local => local::create_file(name, path::Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(_) => unimplemented!(),
    }
}

pub async fn create_dir(name: &str, dir: &FileId) -> anyhow::Result<FileId> {
    let FileId(source, id) = dir;
    match source {
        FileSource::Local => local::create_dir(name, path::Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(_) => unimplemented!(),
    }
}

pub async fn rename(id: &FileId, new_name: &str) -> anyhow::Result<FileId> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::rename(path::Path::new(id), new_name).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(_) => unimplemented!(),
    }
}

pub async fn mv(file: &FileId, dir: &FileId) -> anyhow::Result<FileId> {
    match (&file.0, &dir.0) {
        (FileSource::Local, FileSource::Local) => {
            local::mv(path::Path::new(&file.1), path::Path::new(&dir.1)).await
        }

        #[cfg(feature = "google_drive")]
        (FileSource::Local, FileSource::GoogleDrive(_)) => unimplemented!(),

        #[cfg(feature = "google_drive")]
        (FileSource::GoogleDrive(_), FileSource::Local) => unimplemented!(),

        #[cfg(feature = "google_drive")]
        (FileSource::GoogleDrive(_), FileSource::GoogleDrive(_)) => unimplemented!(),
    }
}

pub async fn delete_file(file: &FileId) -> anyhow::Result<bool> {
    let FileId(source, id) = file;
    match source {
        FileSource::Local => local::delete_file(path::Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(_) => unimplemented!(),
    }
}

pub async fn delete_dir(dir: &FileId) -> anyhow::Result<bool> {
    let FileId(source, id) = dir;
    match source {
        FileSource::Local => local::delete_dir(path::Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(_) => unimplemented!(),
    }
}

pub async fn get_mime(file: &FileId) -> anyhow::Result<String> {
    let FileId(source, id) = file;
    match source {
        FileSource::Local => local::get_mime(path::Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(_) => unimplemented!(),
    }
}
