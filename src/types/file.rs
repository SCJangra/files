use anyhow::Result;
use futures::Stream;

use crate::*;

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

impl File {
    pub async fn new(file_type: &FileType, name: &str, parent_id: &FileId) -> Result<Self> {
        api::create(file_type, name, parent_id).await
    }

    pub async fn get(file_id: &FileId) -> Result<File> {
        api::get(file_id).await
    }

    pub async fn rename(&mut self, new_name: &str) -> Result<()> {
        api::rename(&self.id, new_name).await?;

        self.name = new_name.into();
        Ok(())
    }

    pub async fn move_to_dir(&mut self, dir_id: &FileId) -> Result<()> {
        api::move_to_dir(&self.id, dir_id).await?;

        self.parent_id = Some(dir_id.to_owned());
        Ok(())
    }

    pub async fn delete(self) -> Result<()> {
        match self.file_type {
            FileType::Dir => api::delete_dir(&self.id).await,
            _ => api::delete_file(&self.id).await,
        }
    }

    pub async fn mime(&self) -> Result<String> {
        api::mime(&self.id).await
    }

    pub async fn reader(&self) -> Result<Reader> {
        Reader::new(self).await
    }

    pub async fn writer(&mut self) -> Result<Writer> {
        Writer::new(self).await
    }

    pub fn list(&self) -> impl Stream<Item = Result<File>> + '_ {
        api::list(&self.id)
    }

    pub fn copy_to_dir<'a>(&'a self, dir_id: &'a FileId) -> impl Stream<Item = Result<u64>> + 'a {
        api::copy_to_dir(&self.id, &self.name, dir_id)
    }
}
