use anyhow::Context;
use reqwest::Response;

use crate::{file_source::google_drive::api::oauth::*, types::google_drive::*};

const RES_URI: &str = "https://www.googleapis.com/drive/v3/files";

lazy_static::lazy_static! {
    static ref GET_FIELDS: String = DriveFile::fields().join(",");
    static ref LIST_FIELDS: String = format!("files({})", GET_FIELDS.as_str());
}

pub async fn get(name: &str, id: &str, media: bool) -> anyhow::Result<Response> {
    let token = get_tokne(name).await?;
    let url = format!("{}/{}", RES_URI, id);

    let req = match media {
        true => HTTP
            .get(&url)
            .query(&[("fields", GET_FIELDS.as_str()), ("alt", "media")]),
        false => HTTP.get(&url).query(&[("fields", GET_FIELDS.as_str())]),
    };

    req.header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .with_context(|| format!("Could not send GET request to `{}`", &url))
}

pub async fn list(
    name: &str,
    parent_id: &str,
    page_token: Option<&str>,
) -> anyhow::Result<Response> {
    let token = get_tokne(name).await?;

    let req = HTTP
        .get(RES_URI)
        .header("Authorization", format!("Bearer {}", token));

    let req = match page_token {
        None => req.query(&[
            ("fields", LIST_FIELDS.as_str()),
            ("q", format!("parents in '{}'", parent_id).as_str()),
            ("pageSize", "1000"),
        ]),
        Some(s) => req.query(&[
            ("fields", LIST_FIELDS.as_str()),
            ("q", format!("parents in '{}'", parent_id).as_str()),
            ("pageSize", "1000"),
            ("pageToken", s),
        ]),
    };

    req.send()
        .await
        .with_context(|| format!("Could not send GET request to `{}`", RES_URI))
}
