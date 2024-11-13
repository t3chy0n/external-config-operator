use std::collections::HashMap;
use std::hash::Hash;
use std::os::linux::raw::stat;
use std::time::Duration;
use async_trait::async_trait;
use log::debug;
use tracing_subscriber::fmt::format;
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
    async fn get_config(&self, query_params: Option<HashMap<String, String>>) -> Result<String, Error> {
        // HTTP client logic (could use reqwest, etc.)
        let client = reqwest::Client::new();

        let url = reqwest::Url::parse_with_params(
            &self.config.url,
            query_params.unwrap_or(HashMap::new())
        ).map_err(|e|  { Error::HttpConfigStoreClientError(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())) })?;

        let response = client
            .get(url.clone())
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e|  { Error::HttpConfigStoreError(e) })?; // Explicitly convert error

        let status_code = response.status();

        let res_txt = response.text().await.map_err(|e|  { Error::HttpConfigStoreError(e) })?;

        if !status_code.is_success() {
            debug!("Extractor responded with error Url: {}, Response: {}, StatusCode: {}", &url.as_str(), res_txt, status_code.as_str());
            // Check if the response status is successful
            if status_code.is_server_error() {
                return Err(Error::HttpConfigStoreServerError(std::io::Error::new(std::io::ErrorKind::Other, res_txt)));
            }
            if status_code.is_client_error() {
                return Err(Error::HttpConfigStoreClientError(std::io::Error::new(std::io::ErrorKind::Other, res_txt)));
            }
        }

        Ok(res_txt)

    }
}
