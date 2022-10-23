use std::path::Path;

use async_stream::{stream, try_stream};
use futures::Stream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::*;

#[cfg(feature = "google_drive")]
use crate::google_drive as gd;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct File {
    pub name: String,
    pub file_type: FileType,
    pub size: u64,
    pub id: FileId,
    pub parent_id: Option<FileId>,
}

use FileSource as FS;
use FileType as FT;

impl File {
    pub async fn new(file_type: &FileType, name: &str, parent_id: &FileId) -> anyhow::Result<Self> {
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

    pub async fn get(id: &FileId) -> anyhow::Result<File> {
        let FileId(source, id) = id;
        match source {
            FS::Local => local::get_meta(Path::new(id)).await,
            #[cfg(feature = "google_drive")]
            FS::GoogleDrive(c) => google_drive::get_meta(c, id).await,
        }
    }

    pub async fn rename(&mut self, new_name: &str) -> anyhow::Result<()> {
        let FileId(source, id) = &self.id;

        match source {
            FS::Local => local::rename(Path::new(id), new_name).await?,
            #[cfg(feature = "google_drive")]
            FS::GoogleDrive(c) => gd::rename(c, id, new_name).await?,
        }

        self.name = new_name.into();
        Ok(())
    }

    pub async fn move_to_dir(&mut self, dir_id: &FileId) -> anyhow::Result<()> {
        match (&self.id.0, &dir_id.0) {
            (FS::Local, FS::Local) => {
                local::mv(Path::new(&self.id.1), Path::new(&dir_id.1)).await?
            }

            #[cfg(feature = "google_drive")]
            (FS::GoogleDrive(current_owner), FS::GoogleDrive(new_owner)) => {
                match current_owner == new_owner {
                    true => gd::mv(current_owner, &self.id.1, &dir_id.1).await?,
                    false => {
                        return Err(anyhow::anyhow!(
                            "moving google drive files across acounts is currently not supported"
                        ))
                    }
                }
            }

            #[allow(unreachable_patterns)]
            _ => {
                return Err(anyhow::anyhow!(
                    "moving files across file sources is currently not supported"
                ))
            }
        };

        self.parent_id = Some(dir_id.to_owned());
        Ok(())
    }

    pub async fn delete(self) -> anyhow::Result<()> {
        let FileId(source, id) = &self.id;

        match source {
            FS::Local => match &self.file_type {
                FT::Dir => local::delete_dir(Path::new(id)).await,
                _ => local::delete_file(Path::new(id)).await,
            },
            #[cfg(feature = "google_drive")]
            FS::GoogleDrive(config) => gd::delete(config, id).await,
        }
    }

    pub async fn mime(&self) -> anyhow::Result<String> {
        let FileId(source, id) = &self.id;
        match source {
            FileSource::Local => local::get_mime(Path::new(id)).await,
            #[cfg(feature = "google_drive")]
            FileSource::GoogleDrive(c) => gd::get_mime(c, id).await,
        }
    }

    pub async fn reader(&self) -> anyhow::Result<Reader> {
        Reader::new(self).await
    }

    pub async fn writer(&mut self) -> anyhow::Result<Writer> {
        Writer::new(self).await
    }

    pub fn list(&self) -> impl Stream<Item = anyhow::Result<File>> + '_ {
        let FileId(source, id) = &self.id;

        stream! {
            match source {
                FileSource::Local => {
                    let s = local::list_meta(Path::new(id));
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

    pub fn copy_to_dir<'a>(
        &'a self,
        dir_id: &'a FileId,
    ) -> impl Stream<Item = anyhow::Result<u64>> + 'a {
        try_stream! {
            let mut f = Self::new(&FileType::File, &self.name, dir_id).await?;
            let (r, w) = futures::future::try_join(self.reader(), f.writer()).await?;

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
}
