use std::{
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::AsyncWrite;

use crate::*;

pub struct Writer<'a> {
    file: &'a mut File,
    inner: BoxedAsyncWrite<'a>,
}

impl<'a> Writer<'a> {
    pub async fn new(file: &'a mut File) -> anyhow::Result<Writer<'a>> {
        let file_id = unsafe { std::mem::transmute::<&FileId, &FileId>(&file.id) };

        let inner: BoxedAsyncWrite = api::write(&file_id).await?;

        file.size = 0;
        Ok(Self { file, inner })
    }
}

impl<'a> AsyncWrite for Writer<'a> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();

        match this.inner.as_mut().poll_write(cx, buf) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                let written = result?;
                this.file.size += written as u64;
                Ok(written).into()
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();

        this.inner.as_mut().poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();

        this.inner.as_mut().poll_shutdown(cx)
    }
}
