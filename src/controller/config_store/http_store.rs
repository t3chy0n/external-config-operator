use std::collections::HashMap;
use async_trait::async_trait;
use crate::contract::iconfigstore::IConfigStore;
use crate::contract::lib::Error;

pub struct HttpConfigStoreConnectionDetails {
    pub url: String,
    pub headers: HashMap<String, String>,
    pub query_params: HashMap<String, String>
}

pub struct HttpConfigStore {
    config: HttpConfigStoreConnectionDetails,
}

impl HttpConfigStore {
    pub fn new(config: HttpConfigStoreConnectionDetails) -> Self {
        HttpConfigStore { config }
    }
}

#[async_trait]
impl IConfigStore for HttpConfigStore {
    async fn get_config(&self, key: String) -> Result<String, Error> {
        // HTTP client logic (could use reqwest, etc.)
        let client = reqwest::Client::new();

        let response = client
            .get(&self.config.url)
            .send()
            .await
            .map_err(|e|  { Error::HttpConfigStoreError(e) })?; // Explicitly convert error

        let res_txt = response.text().await.map_err(|e|  { Error::HttpConfigStoreError(e) })?;

        Ok(res_txt)

    }
}
