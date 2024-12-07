use crate::contract::iconfigstore::IConfigStore;
use crate::contract::lib::Error;
use async_trait::async_trait;
use log::debug;
use reqwest::header;
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use std::hash::Hash;
use std::os::linux::raw::stat;
use std::time::Duration;
use tracing_subscriber::fmt::format;

pub struct HttpConfigStoreConnectionDetails {
    pub base_url: String,
    pub path: Option<String>,
    pub protocol: Option<String>,
    pub headers: HashMap<String, String>,
    pub query_params: HashMap<String, String>,
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
    async fn get_config(
        &self,
        query_params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<String, Error> {
        let mut merged_headers_map: HashMap<String, String> = HashMap::new();
        merged_headers_map.extend(self.config.headers.clone());
        merged_headers_map.extend(headers.unwrap_or(HashMap::new()));

        let mut merged_query_params: HashMap<String, String> = HashMap::new();
        merged_query_params.extend(self.config.query_params.clone());
        merged_query_params.extend(query_params.unwrap_or(HashMap::new()));

        let headers: HeaderMap = (&merged_headers_map).try_into().expect("Valid headers");

        let client = reqwest::Client::new();

        let protocol = &self.config.protocol.clone().unwrap_or(String::from("http"));
        let path = &self.config.path.clone().unwrap_or(String::from(""));
        let base_url = &self.config.base_url.clone();

        let processed_url = format!("{}://{}{}", protocol, base_url, path,);

        let url = reqwest::Url::parse_with_params(processed_url.as_str(), merged_query_params)
            .map_err(|e| {
                Error::HttpConfigStoreClientError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;

        let response = client
            .get(url.clone())
            .headers(headers)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| Error::HttpConfigStoreError(e))?; // Explicitly convert error

        let status_code = response.status();

        let res_txt = response
            .text()
            .await
            .map_err(|e| Error::HttpConfigStoreError(e))?;

        if !status_code.is_success() {
            debug!(
                "Extractor responded with error Url: {}, Response: {}, StatusCode: {}",
                &url.as_str(),
                res_txt,
                status_code.as_str()
            );
            // Check if the response status is successful
            if status_code.is_server_error() {
                return Err(Error::HttpConfigStoreServerError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    res_txt,
                )));
            }
            if status_code.is_client_error() {
                return Err(Error::HttpConfigStoreClientError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    res_txt,
                )));
            }
        }

        Ok(res_txt)
    }
}
