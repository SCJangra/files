use anyhow::Context;

use crate::{file_source::google_drive::api::oauth::*, types::google_drive::*};

const RES_URI: &str = "https://www.googleapis.com/drive/v3/files";

lazy_static::lazy_static! {
    static ref GET_FIELDS: String = DriveFile::fields().join(",");
    static ref LIST_FIELDS: String = format!("files({})", GET_FIELDS.as_str());
}

pub async fn get(name: &str, id: &str) -> anyhow::Result<DriveFile> {
    let token = get_tokne(name).await?;
    let url = format!("{}/{}", RES_URI, id);

    let res: Res = HTTP
        .get(&url)
        .query(&[("fields", GET_FIELDS.as_str())])
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .with_context(|| format!("Could not send GET request to `{}`", &url))?
        .into();
    res.json::<DriveFile>().await
}

pub async fn list(name: &str, parent_id: &str) -> anyhow::Result<impl Iterator<Item = DriveFile>> {
    let token = get_tokne(name).await?;

    let mut files = vec![];
    let mut next_page_token: Option<String> = None;

    loop {
        let req = HTTP
            .get(RES_URI)
            .header("Authorization", format!("Bearer {}", token));

        let req = match next_page_token {
            None => req.query(&[
                ("fields", LIST_FIELDS.as_str()),
                ("q", format!("parents in '{}'", parent_id).as_str()),
                ("pageSize", "1000"),
            ]),
            Some(s) => req.query(&[
                ("fields", LIST_FIELDS.as_str()),
                ("q", format!("parents in '{}'", parent_id).as_str()),
                ("pageSize", "1000"),
                ("pageToken", s.as_str()),
            ]),
        };

        let res: Res = req
            .send()
            .await
            .with_context(|| format!("Could not send GET request to `{}`", RES_URI))?
            .into();

        let list = res.json::<ListResponse>().await?;

        files.push(list.files);

        match list.next_page_token {
            None => break,
            Some(s) => next_page_token = Some(s),
        }
    }

    Ok(files.into_iter().flatten())
}
