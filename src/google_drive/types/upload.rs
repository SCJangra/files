use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::Result;
use futures::{future::BoxFuture, Future, FutureExt};
use reqwest::header::*;
use tokio::io::AsyncWrite;

use crate::google_drive::{
    oauth::get_auth_header,
    utils::{parse_range_header, IntoIOErr},
    HTTP,
};

// 512 KB
const BUF_SIZE: usize = (256 * 1024) * 2;

pub struct Upload<'a> {
    upload_url: String,
    config_name: &'a str,
    sent: u64,
    buf: Vec<u8>,
    state: State<'a>,
}

enum State<'a> {
    // in this state the buffer is never full
    Buffering,
    // in this state the buffer is always full
    Uploading(BoxFuture<'a, Result<u64>>),
}

impl<'a> Upload<'a> {
    pub fn new(upload_url: String, config_name: &'a str) -> Upload<'a> {
        Self {
            upload_url,
            config_name,
            sent: 0,
            buf: Vec::with_capacity(BUF_SIZE),
            state: State::Buffering,
        }
    }

    fn upload(&mut self, size: Option<u64>) -> impl Future<Output = Result<u64>> {
        let upload_url = unsafe { &*std::ptr::addr_of!(*self.upload_url) };
        let config_name = unsafe { &*std::ptr::addr_of!(*self.config_name) };
        let buf = unsafe { &*std::ptr::addr_of!(self.buf) };
        let sent = unsafe { &mut *std::ptr::addr_of_mut!(self.sent) };

        let len = buf.len() as u64;
        let range_start = self.sent;
        let range_end = (range_start + len) - 1;
        let content_range = match size {
            None => format!("bytes {range_start}-{range_end}/*"),
            Some(size) => format!("bytes {range_start}-{range_end}/{size}"),
        };

        async move {
            let res = HTTP
                .put(upload_url)
                .header(AUTHORIZATION, get_auth_header(config_name).await?)
                .header(CONTENT_LENGTH, len)
                .header(CONTENT_RANGE, content_range)
                .body(&buf[..])
                .send()
                .await?
                .error_for_status()?;

            if res.status().is_success() {
                let s = buf.len() as u64;
                *sent += s;

                return Ok(s);
            }

            let range = res
                .headers()
                .get(RANGE)
                .ok_or_else(|| anyhow::anyhow!("unexpected response with no range header"))?
                .to_str()?;
            let (start, end) = parse_range_header(range)?;
            *sent = end + 1;

            Ok(end - start + 1)
        }
    }

    fn write_to_buf(&mut self, src: &[u8]) -> Result<u64> {
        let filled = self.buf.len();

        if filled >= BUF_SIZE {
            return Err(anyhow::anyhow!("buffer is already filled"));
        }

        let src_len = src.len();
        let free = BUF_SIZE - filled;
        let write = if src_len > free { free } else { src_len };
        self.buf.extend_from_slice(&src[..write]);

        Ok(write as u64)
    }

    fn is_buffer_full(&self) -> bool {
        self.buf.len() >= BUF_SIZE
    }
}

impl<'a> AsyncWrite for Upload<'a> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        src: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut this = self.get_mut();

        match &mut this.state {
            State::Buffering => {
                let last_written = this.write_to_buf(src).map_err(|e| e.into_io_err())?;

                if this.is_buffer_full() {
                    this.state = State::Uploading(this.upload(None).boxed());
                }

                Ok(last_written as usize).into()
            }

            State::Uploading(fut) => match fut.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(result) => {
                    let sent = result.map_err(|e| e.into_io_err())?;
                    this.buf.drain(..sent as usize);
                    this.state = State::Buffering;

                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            },
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Ok(()).into()
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        match &mut this.state {
            State::Buffering => {
                let size = this.sent + this.buf.len() as u64;
                this.state = State::Uploading(this.upload(Some(size)).boxed());
                cx.waker().wake_by_ref();
                Poll::Pending
            }

            State::Uploading(fut) => match fut.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(result) => {
                    let sent = result.map_err(|e| e.into_io_err())?;
                    this.buf.drain(..sent as usize);

                    if this.buf.is_empty() {
                        this.state = State::Buffering;
                        return Ok(()).into();
                    }

                    let size = this.sent + this.buf.len() as u64;
                    this.state = State::Uploading(this.upload(Some(size)).boxed());
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            },
        }
    }
}
