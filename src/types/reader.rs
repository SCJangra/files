use std::pin::Pin;

use tokio::io::AsyncRead;

use crate::*;

pub struct Reader<'a> {
    _file: &'a File,
    inner: BoxedAsyncRead,
}

impl<'a> Reader<'a> {
    pub async fn new(file: &'a File) -> anyhow::Result<Reader<'a>> {
        let inner: BoxedAsyncRead = api::read(&file.id).await?;

        Ok(Self { _file: file, inner })
    }
}

impl<'a> AsyncRead for Reader<'a> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        this.inner.as_mut().poll_read(cx, buf)
    }
}
