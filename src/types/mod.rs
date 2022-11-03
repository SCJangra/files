mod file;
mod reader;
mod writer;
pub use file::File;
pub use reader::Reader;
pub use writer::Writer;

use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncWrite};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub type BoxedAsyncRead<'a> = Pin<Box<dyn AsyncRead + Send + 'a>>;
pub type BoxedAsyncWrite<'a> = Pin<Box<dyn AsyncWrite + Send + 'a>>;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FileType {
    File,
    Dir,
    Unknown,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FileId(pub FileSource, pub String);

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FileSource {
    Local,
    #[cfg(feature = "google_drive")]
    GoogleDrive(String),
}
