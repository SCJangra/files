use anyhow::Context;

use crate::{file_source::google_drive::api::oauth::*, types::google_drive::*};

const RES_URI: &str = "https://www.googleapis.com/drive/v3/files";

pub async fn get(name: &str, id: &str) -> anyhow::Result<DriveFile> {
    let token = get_tokne(name).await?;
    let url = format!("{}/{}", RES_URI, id);
    let fields = DriveFile::fields().join(",");

    let res = HTTP
        .get(&url)
        .query(&[("fields", fields.as_str())])
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .with_context(|| format!("Could not send GET request to `{}`", &url))?
        // TODO: Do error handeling here
        .text()
        .await
        .with_context(|| "Could not get file meta")?;

    serde_json::from_str::<DriveFile>(&res).with_context(|| "Could not parse drive response")
}
