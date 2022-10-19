use crate::google_drive::CONFIGS;

pub async fn get_auth_header(name: &str) -> anyhow::Result<String> {
    let c = CONFIGS.read().await;
    let config = c
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("A config with name {} does not exist", name))?;

    if config.is_valid()? {
        return Ok(format!("Bearer {}", config.access_token));
    }

    {
        // dropping this read guard is necessary to obtain a write guard
        drop(c);
        let mut c = CONFIGS.write().await;
        let config = c.get_mut(name).unwrap();
        config.refresh().await?;

        Ok(format!("Bearer {}", config.access_token))
    }
}
