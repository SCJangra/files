use crate::{file_source::google_drive::CONFIGS, types::google_drive::Config};

pub async fn google_drive(
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
