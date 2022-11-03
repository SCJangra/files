use crate::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use FileSource as FS;
use FileType as FT;

#[cfg(feature = "google_drive")]
use crate::google_drive as gd;

use std::path::Path;

use anyhow::Result;
use async_stream::{stream, try_stream};
use futures::Stream;

pub async fn create(file_type: &FT, name: &str, parent_id: &FileId) -> Result<File> {
    let FileId(source, parent_id) = parent_id;

    match (source, file_type) {
        (FS::Local, FT::File) => local::create_file(name, Path::new(parent_id)).await,
        (FS::Local, FT::Dir) => local::create_dir(name, Path::new(parent_id)).await,

        #[cfg(feature = "google_drive")]
        (FS::GoogleDrive(config), FT::File) => gd::create_file(config, name, parent_id).await,
        #[cfg(feature = "google_drive")]
        (FS::GoogleDrive(config), FT::Dir) => gd::create_dir(config, name, parent_id).await,

        _ => Err(anyhow::anyhow!(
            "creating this file type is currently not supported"
        )),
    }
}

pub async fn get(file_id: &FileId) -> Result<File> {
    let FileId(source, id) = file_id;
    match source {
        FS::Local => local::get_meta(Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FS::GoogleDrive(c) => google_drive::get_meta(c, id).await,
    }
}

pub async fn rename(file_id: &FileId, new_name: &str) -> Result<()> {
    let FileId(source, id) = file_id;

    match source {
        FS::Local => local::rename(Path::new(id), new_name).await,
        #[cfg(feature = "google_drive")]
        FS::GoogleDrive(c) => gd::rename(c, id, new_name).await,
    }
}

pub async fn delete_file(file_id: &FileId) -> Result<()> {
    let FileId(source, id) = file_id;

    match source {
        FS::Local => local::delete_file(Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FS::GoogleDrive(config) => gd::delete(config, id).await,
    }
}

pub async fn delete_dir(dir_id: &FileId) -> Result<()> {
    let FileId(source, id) = dir_id;

    match source {
        FS::Local => local::delete_dir(Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FS::GoogleDrive(config) => gd::delete(config, id).await,
    }
}

pub async fn move_to_dir(file_id: &FileId, dir_id: &FileId) -> Result<()> {
    match (&file_id.0, &dir_id.0) {
        (FS::Local, FS::Local) => local::mv(Path::new(&file_id.1), Path::new(&dir_id.1)).await,

        #[cfg(feature = "google_drive")]
        (FS::GoogleDrive(current_owner), FS::GoogleDrive(new_owner)) => {
            match current_owner == new_owner {
                true => gd::mv(current_owner, &file_id.1, &dir_id.1).await,
                false => Err(anyhow::anyhow!(
                    "moving google drive files across acounts is currently not supported"
                )),
            }
        }

        #[allow(unreachable_patterns)]
        _ => Err(anyhow::anyhow!(
            "moving files across file sources is currently not supported"
        )),
    }
}

pub async fn mime(file_id: &FileId) -> Result<String> {
    let FileId(source, id) = &file_id;
    match source {
        FS::Local => local::get_mime(Path::new(id)).await,
        #[cfg(feature = "google_drive")]
        FS::GoogleDrive(c) => gd::get_mime(c, id).await,
    }
}

pub fn list(dir_id: &FileId) -> impl Stream<Item = Result<File>> + '_ {
    let FileId(source, id) = dir_id;

    stream! {
        match source {
            FS::Local => {
                let s = local::list_meta(Path::new(id));
                for await v in s {
                    yield v;
                }
            }
            #[cfg(feature = "google_drive")]
            FS::GoogleDrive(name) => {
                let s = gd::list_meta(name, id);
                for await v in s {
                    yield v;
                }
            },
        }
    }
}

pub fn copy_to_dir<'a>(
    file_id: &'a FileId,
    name: &'a str,
    dir_id: &'a FileId,
) -> impl Stream<Item = Result<u64>> + 'a {
    try_stream! {
        let f = create(&FT::File, name, dir_id).await?;
        let (r, w) = futures::future::try_join(read(file_id), write(&f.id)).await?;

        let mut reader = tokio::io::BufReader::new(r);
        let mut writer = tokio::io::BufWriter::new(w);

        let mut buf = vec!(0u8; 5 * 1024 * 1024);

        loop {
            let bytes = async {
                let bytes = reader.read(&mut buf).await?;

                writer.write_all(&buf[..bytes]).await?;

                anyhow::Ok(bytes)
            }
            .await?;

            if bytes == 0 {
                break;
            }

            yield bytes as u64;
        }
        writer.shutdown().await?;
    }
}

pub(crate) async fn read(file_id: &FileId) -> Result<BoxedAsyncRead> {
    let FileId(source, id) = &file_id;

    let r: BoxedAsyncRead = match source {
        FS::Local => local::read(Path::new(id)).await.map(Box::pin)?,
        #[cfg(feature = "google_drive")]
        FS::GoogleDrive(c) => google_drive::read(c, id).await.map(Box::pin)?,
    };

    Ok(r)
}

pub(crate) async fn write(file_id: &FileId) -> Result<BoxedAsyncWrite> {
    let FileId(source, id) = &file_id;

    let w: BoxedAsyncWrite = match source {
        FS::Local => local::write(Path::new(id)).await.map(Box::pin)?,
        #[cfg(feature = "google_drive")]
        FS::GoogleDrive(c) => google_drive::write(c, id).await.map(Box::pin)?,
    };

    Ok(w)
}
