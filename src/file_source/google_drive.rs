mod api;

use crate::types::{google_drive::*, *};
pub use api::oauth::CONFIGS;
use async_stream::try_stream;
use futures::Stream;

pub async fn get_meta(name: &str, id: &str) -> anyhow::Result<FileMeta> {
    let res: Res = api::res::files::get(name, id, false).await?.into();

    let m = res.json::<DriveFile>().await?;

    Ok((m, name).into())
}

pub fn list_meta<'a>(
    name: &'a str,
    parent_id: &'a str,
) -> impl Stream<Item = anyhow::Result<FileMeta>> + 'a {
    let mut next_page_token: Option<String> = None;

    try_stream! {
        loop {
            let res: Res = api::res::files::list(name, parent_id, next_page_token.as_deref()).await?.into();
            let res = res.json::<ListResponse>().await?;

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
