pub mod api;
pub mod types;

mod oauth;
mod res;

use std::collections::HashMap;

use reqwest::Client;
use tokio::sync::RwLock;

use types::Config;

lazy_static::lazy_static! {
    pub static ref CONFIGS: RwLock<HashMap<String, Config>> = RwLock::new(HashMap::<String, Config>::new());
    pub static ref HTTP: Client = Client::new();
}
