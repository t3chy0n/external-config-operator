use std::collections::HashMap;
use async_trait::async_trait;
use crate::contract::iconfigstore::IConfigStore;
use crate::contract::lib::Error;

pub struct VaultConfigStoreConnectionDetails {
    pub url: String,
    pub headers: HashMap<String, String>,
    pub query_params: HashMap<String, String>
}

pub struct VaultConfigStore {
    config: VaultConfigStoreConnectionDetails,
}

impl VaultConfigStore {
    pub fn new(config: VaultConfigStoreConnectionDetails) -> Self {
        VaultConfigStore { config }
    }
}
#[async_trait]
impl IConfigStore for VaultConfigStore {
    async fn get_config(
        &self,
        query_params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>
    ) -> Result<String, Error> {
        // HTTP client logic (could use reqwest, etc.)

        println!("Fetching config from HTTP at {}", self.config.url );
        Ok(String::from("Vault config"))
    }
}
