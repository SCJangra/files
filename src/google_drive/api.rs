use crate::{
    google_drive::{types::*, CONFIGS},
    *,
};

use async_stream::try_stream;
use futures::{Stream, TryStreamExt};
use reqwest::{header::*, Response};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::compat::FuturesAsyncReadCompatExt;

use super::{oauth, HTTP};

pub const RES_URI: &str = "https://www.googleapis.com/drive/v3/files";
pub const UPLOAD_URI: &str = "https://www.googleapis.com/upload/drive/v3/files";

lazy_static::lazy_static! {
    static ref GET_FIELDS: String = DriveFile::fields().join(",");
    static ref LIST_FIELDS: String = format!("files({})", GET_FIELDS.as_str());
}

pub async fn get_meta(config_name: &str, id: &str) -> anyhow::Result<File> {
    let f = HTTP
        .get(&format!("{RES_URI}/{id}"))
        .query(&[("fields", GET_FIELDS.as_str())])
        .header(AUTHORIZATION, &oauth::get_auth_header(config_name).await?)
        .send()
        .await?
        .json::<DriveFile>()
        .await?;

    Ok((f, config_name).into())
}

pub async fn read(config_name: &str, id: &str) -> anyhow::Result<impl AsyncRead> {
    let s = HTTP
        .get(&format!("{RES_URI}/{id}"))
        .query(&[("alt", "media")])
        .header(AUTHORIZATION, &oauth::get_auth_header(config_name).await?)
        .send()
        .await?
        .bytes_stream()
        .map_err(|e| futures::io::Error::new(futures::io::ErrorKind::Other, e))
        .into_async_read()
        .compat();

    Ok(s)
}

pub async fn write<'a>(config_name: &'a str, id: &'a str) -> anyhow::Result<impl AsyncWrite + 'a> {
    let upload_url = HTTP
        .patch(&format!("{UPLOAD_URI}/{id}"))
        .query(&[("uploadType", "resumable")])
        .header(AUTHORIZATION, &oauth::get_auth_header(config_name).await?)
        .send()
        .await?
        .error_for_status()?
        .headers()
        .get(LOCATION)
        .ok_or_else(|| anyhow::anyhow!("unexpected response with no `Location` header"))?
        .to_str()?
        .to_owned();

    Ok(Upload::new(upload_url, config_name))
}

pub fn list_meta<'a>(
    config_name: &'a str,
    parent_id: &'a str,
) -> impl Stream<Item = anyhow::Result<File>> + 'a {
    let mut next_page_token: Option<String> = None;

    try_stream! {
        loop {
            let res = list(config_name, parent_id, next_page_token.as_deref())
                .await?
                .json::<ListResponse>()
                .await?;

            for f in res.files.into_iter() {
                yield File::from((f, config_name));
            }

            match res.next_page_token {
                None => break,
                Some(t) => next_page_token = Some(t),
            };
        }
    }
}

pub async fn create_file(
    config_name: &str,
    file_name: &str,
    parent_dir: &str,
) -> anyhow::Result<File> {
    let f = HTTP
        .post(RES_URI)
        .header(AUTHORIZATION, &oauth::get_auth_header(config_name).await?)
        .json(&serde_json::json!({
            "name": file_name,
            "parents": [parent_dir],
        }))
        .send()
        .await?
        .json::<DriveFile>()
        .await?;

    Ok((f, config_name).into())
}

pub async fn create_dir(
    config_name: &str,
    dir_name: &str,
    parent_dir: &str,
) -> anyhow::Result<File> {
    let f = HTTP
        .post(RES_URI)
        .header(AUTHORIZATION, &oauth::get_auth_header(config_name).await?)
        .json(&serde_json::json!({
            "name": dir_name,
            "parents": [parent_dir],
            "mimeType": "application/vnd.google-apps.folder"
        }))
        .send()
        .await?
        .json::<DriveFile>()
        .await?;

    Ok((f, config_name).into())
}

pub async fn rename(config_name: &str, id: &str, new_name: &str) -> anyhow::Result<()> {
    HTTP.patch(&format!("{RES_URI}/{id}"))
        .header(AUTHORIZATION, &oauth::get_auth_header(config_name).await?)
        .json(&serde_json::json!({ "name": new_name }))
        .send()
        .await?
        .error_for_status()
        .map(|_r| ())
        .map_err(anyhow::Error::new)
}

pub async fn mv(config_name: &str, id: &str, new_parent: &str) -> anyhow::Result<()> {
    HTTP.patch(&format!("{RES_URI}/{id}"))
        .query(&[("addParents", new_parent)])
        .header(AUTHORIZATION, &oauth::get_auth_header(config_name).await?)
        .json("{}")
        .send()
        .await?
        .error_for_status()
        .map(|_r| ())
        .map_err(anyhow::Error::new)
}

pub async fn delete(config_name: &str, id: &str) -> anyhow::Result<()> {
    HTTP.delete(&format!("{RES_URI}/{id}"))
        .header(AUTHORIZATION, &oauth::get_auth_header(config_name).await?)
        .send()
        .await?
        .error_for_status()
        .map_err(anyhow::Error::new)
        .map(|_r| ())
}

pub async fn get_mime(config_name: &str, id: &str) -> anyhow::Result<String> {
    let f = HTTP
        .get(&format!("{RES_URI}/{id}"))
        .query(&[("fields", GET_FIELDS.as_str())])
        .header(AUTHORIZATION, &oauth::get_auth_header(config_name).await?)
        .send()
        .await?
        .error_for_status()?
        .json::<DriveFile>()
        .await?;

    Ok(f.mime_type)
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

async fn list(name: &str, parent_id: &str, page_token: Option<&str>) -> anyhow::Result<Response> {
    let req = HTTP
        .get(RES_URI)
        .header(AUTHORIZATION, &oauth::get_auth_header(name).await?);

    let req = match page_token {
        None => req.query(&[
            ("fields", LIST_FIELDS.as_str()),
            ("q", format!("parents in '{parent_id}'").as_str()),
            ("pageSize", "1000"),
        ]),
        Some(s) => req.query(&[
            ("fields", LIST_FIELDS.as_str()),
            ("q", format!("parents in '{parent_id}'").as_str()),
            ("pageSize", "1000"),
            ("pageToken", s),
        ]),
    };

    req.send().await.map_err(anyhow::Error::new)
}
