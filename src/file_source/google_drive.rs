mod api;

use crate::types::*;
pub use api::oauth::CONFIGS;
use async_stream::stream;
use futures::Stream;
use unwrap_or::unwrap_ok_or;

pub async fn get_meta(name: &str, id: &str) -> anyhow::Result<FileMeta> {
    let df = api::res::files::get(name, id).await?;

    Ok((df, name).into())
}

pub fn list_meta<'a>(
    name: &'a str,
    parent_id: &'a str,
) -> impl Stream<Item = anyhow::Result<FileMeta>> + 'a {
    stream! {
        let files = api::res::files::list(name, parent_id).await;
        let files = unwrap_ok_or!(files, e, {
            yield Err(e);
            return;
        });
        let files = files.map(move |f| FileMeta::from((f, name)));

        for f in files { yield Ok(f); }
    }
}
