use anyhow::Context;
use async_stream::{stream, try_stream};
use futures::Stream;
use std::{path, pin::Pin};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, BufWriter};

#[cfg(feature = "google_drive")]
use crate::google_drive::api as gd;

use crate::{local::api as local, types::*};

type DynAsyncRead = Pin<Box<dyn AsyncRead + Send>>;
type DynAsyncWrite = Pin<Box<dyn AsyncWrite + Send>>;

pub async fn get_meta(id: &FileId) -> anyhow::Result<FileMeta> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::get_meta(path::Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(name) => gd::get_meta(name, id).await,
    }
}

pub fn list_meta(id: &FileId) -> impl Stream<Item = anyhow::Result<FileMeta>> + '_ {
    let FileId(source, id) = id;

    stream! {
        match source {
            FileSource::Local => {
                let s = local::list_meta(path::Path::new(id));
                for await v in s {
                    yield v;
                }
            }
            #[cfg(feature = "google_drive")]
            FileSource::GoogleDrive(name) => {
                let s = gd::list_meta(name, id);
                for await v in s {
                    yield v;
                }
            },
        }
    }
}

pub async fn read(id: &FileId) -> anyhow::Result<DynAsyncRead> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::read(path::Path::new(id))
            .await
            .map(|r| -> DynAsyncRead { Box::pin(r) }),
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(name) => gd::read(name, id)
            .await
            .map(|r| -> DynAsyncRead { Box::pin(r) }),
    }
}

pub async fn write(id: &FileId) -> anyhow::Result<DynAsyncWrite> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::write(path::Path::new(id))
            .await
            .map(|w| -> DynAsyncWrite { Box::pin(w) }),
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(_) => unimplemented!(),
    }
}

pub async fn create_file(name: &str, dir: &FileId) -> anyhow::Result<FileId> {
    let FileId(source, id) = dir;
    match source {
        FileSource::Local => local::create_file(name, path::Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(c) => gd::create_file(c, name, id).await,
    }
}

pub async fn create_dir(name: &str, dir: &FileId) -> anyhow::Result<FileId> {
    let FileId(source, id) = dir;
    match source {
        FileSource::Local => local::create_dir(name, path::Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(c) => gd::create_dir(c, name, id).await,
    }
}

pub async fn rename(id: &FileId, new_name: &str) -> anyhow::Result<FileId> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::rename(path::Path::new(id), new_name).await,
        #[cfg(feature = "google_drive")]
        FileSource::GoogleDrive(c) => gd::rename(c, id, new_name).await,
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

pub fn copy_file<'a>(
    src: &'a FileId,
    dst_dir: &'a FileId,
    name: &'a str,
) -> impl Stream<Item = anyhow::Result<u64>> + 'a {
    try_stream! {
        let dst = create_file(name, dst_dir).await?;
        let (rd, wr) = futures::future::try_join(read(src), write(&dst)).await?;

        let mut reader = BufReader::new(rd);
        let mut writer = BufWriter::new(wr);

        let mut buf = vec![0u8; 10_000_000];

        loop {
            let bytes = async {
                let bytes = reader
                    .read(&mut buf)
                    .await
                    .with_context(|| format!("Error while reading file {}", name))?;

                writer
                    .write_all(&buf[..bytes])
                    .await
                    .with_context(|| format!("Error while writing to file {}", name))?;

                anyhow::Ok(bytes)
            }
            .await?;

            if bytes == 0 {
                break;
            }

            yield bytes as u64;
        }
        writer.flush().await?;
    }
}
