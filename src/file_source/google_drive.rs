mod api;

use crate::types::*;
pub use api::oauth::CONFIGS;

pub async fn get_meta(name: &str, id: &str) -> anyhow::Result<FileMeta> {
    let df = api::res::files::get(name, id).await?;

    Ok((df, name).into())
}
