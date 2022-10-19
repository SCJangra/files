use crate::{
    google_drive::{res, types::*, CONFIGS},
    types::*,
};

use async_stream::try_stream;
use futures::{Stream, TryStreamExt};
use tokio::io::AsyncRead;
use tokio_util::compat::FuturesAsyncReadCompatExt;

pub async fn get_meta(name: &str, id: &str) -> anyhow::Result<FileMeta> {
    let m = res::files::get(name, id, false)
        .await?
        .json::<DriveFile>()
        .await?;

    Ok((m, name).into())
}

pub async fn read(name: &str, id: &str) -> anyhow::Result<impl AsyncRead> {
    let s = res::files::get(name, id, true)
        .await?
        .bytes_stream()
        .map_err(|e| futures::io::Error::new(futures::io::ErrorKind::Other, e))
        .into_async_read();

    Ok(s.compat())
}

pub fn list_meta<'a>(
    name: &'a str,
    parent_id: &'a str,
) -> impl Stream<Item = anyhow::Result<FileMeta>> + 'a {
    let mut next_page_token: Option<String> = None;

    try_stream! {
        loop {
            let res = res::files::list(name, parent_id, next_page_token.as_deref())
                .await?
                .json::<ListResponse>()
                .await?;

            for f in res.files.into_iter() {
                yield FileMeta::from((f, name));
            }

            match res.next_page_token {
                None => break,
                Some(t) => next_page_token = Some(t),
            };
        }
    }
}

pub async fn add_config(
    name: String,
    client_id: String,
    client_secret: String,
    refresh_token: String,
) {
    CONFIGS.write().await.insert(
        name,
        Config {
            client_id,
            client_secret,
            refresh_token,
            access_token: "".into(),
            expires_at: 0,
        },
    );
}
