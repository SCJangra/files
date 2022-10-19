use anyhow::Context;
use reqwest::Response;

use crate::google_drive::{oauth::*, types::*, HTTP};

const RES_URI: &str = "https://www.googleapis.com/drive/v3/files";

lazy_static::lazy_static! {
    static ref GET_FIELDS: String = DriveFile::fields().join(",");
    static ref LIST_FIELDS: String = format!("files({})", GET_FIELDS.as_str());
}

pub async fn get(name: &str, id: &str, media: bool) -> anyhow::Result<Response> {
    let url = format!("{}/{}", RES_URI, id);

    let req = match media {
        true => HTTP
            .get(&url)
            .query(&[("fields", GET_FIELDS.as_str()), ("alt", "media")]),
        false => HTTP.get(&url).query(&[("fields", GET_FIELDS.as_str())]),
    };

    req.header("Authorization", &get_auth_header(name).await?)
        .send()
        .await
        .with_context(|| format!("Could not send GET request to `{}`", &url))
}

pub async fn list(
    name: &str,
    parent_id: &str,
    page_token: Option<&str>,
) -> anyhow::Result<Response> {
    let req = HTTP
        .get(RES_URI)
        .header("Authorization", &get_auth_header(name).await?);

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

pub async fn create(
    name: &str,
    file_name: &str,
    parent_id: &str,
    upload_type: Option<&str>,
) -> anyhow::Result<Response> {
    let req = match upload_type {
        None => HTTP.post(RES_URI),
        Some(u) => HTTP.post(RES_URI).query(&[("uploadType", u)]),
    };

    req.header("Authorization", &get_auth_header(name).await?)
        .json(&serde_json::json!({
            "name": file_name,
            "parent_id": parent_id,
        }))
        .send()
        .await
        .with_context(|| format!("Could not send POST request to `{}`", RES_URI))
}

pub async fn delete(name: &str, id: &str) -> anyhow::Result<Response> {
    HTTP.delete(format!("{RES_URI}/{id}"))
        .header("Authorization", &get_auth_header(name).await?)
        .send()
        .await
        .with_context(|| format!("Could not send DELETE request to `{RES_URI}/{id}`"))
}

#[cfg(test)]
mod tests {
    use crate::google_drive::{types::*, CONFIGS};

    const CONFIG_NAME: &str = "test";

    async fn setup() -> anyhow::Result<()> {
        let gd_config = "gd_config.json";

        let config = tokio::fs::read_to_string(gd_config).await?;
        let mut config = serde_json::from_str::<Config>(config.as_str())?;

        if !config.is_valid()? {
            config.refresh().await?;

            let config_json = serde_json::to_string_pretty(&serde_json::json!({
                "client_id": &config.client_id,
                "client_secret": &config.client_secret,
                "refresh_token": &config.refresh_token,
                "access_token": &config.access_token,
                "expires_at": &config.expires_at
            }))?;
            tokio::fs::write(gd_config, config_json).await?;
        }

        CONFIGS
            .write()
            .await
            .insert(CONFIG_NAME.to_string(), config);
        Ok(())
    }

    #[tokio::test]
    async fn t() -> anyhow::Result<()> {
        setup().await?;

        let file_name = "some_file.txt";
        let parent_id = "root";

        // Create File
        let f = super::create(CONFIG_NAME, file_name, parent_id, None)
            .await?
            .json::<DriveFile>()
            .await?;
        assert_eq!(f.name.as_str(), file_name);

        // Get File
        let f2 = super::get(CONFIG_NAME, f.id.as_str(), false)
            .await?
            .json::<DriveFile>()
            .await?;
        assert_eq!(f2.id, f.id);
        assert_eq!(f2.name, f.name);

        // List Files
        let l = super::list(CONFIG_NAME, parent_id, None)
            .await?
            .json::<ListResponse>()
            .await?;
        assert!(!l.files.is_empty());

        // Delete File
        let res = super::delete(CONFIG_NAME, f.id.as_str()).await?;
        assert!(res.status().is_success());

        Ok(())
    }
}
