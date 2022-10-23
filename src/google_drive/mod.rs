mod api;
mod oauth;
mod types;
mod utils;

use std::collections::HashMap;

use reqwest::Client;
use tokio::sync::RwLock;

pub use api::*;
pub use types::*;

lazy_static::lazy_static! {
    pub static ref CONFIGS: RwLock<HashMap<String, Config>> = RwLock::new(HashMap::<String, Config>::new());
    pub static ref HTTP: Client = Client::new();
}
