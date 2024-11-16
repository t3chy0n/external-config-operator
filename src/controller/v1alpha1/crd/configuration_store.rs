use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use kube::{Client, CustomResource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::contract::iconfigstore::IConfigStore;
use crate::contract::lib::Error;
use crate::controller::config_store::http_store::{HttpConfigStore, HttpConfigStoreConnectionDetails};
use crate::controller::config_store::vault_store::{VaultConfigStore, VaultConfigStoreConnectionDetails};

// Define the config_store enum
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Provider {
    Http(HttpConfig),
    Vault(VaultConfig),
}

impl Provider {
    pub fn get_config_store(&self) -> Box<dyn IConfigStore> {
        match &self {
            Provider::Http(http_config) => Box::new(HttpConfigStore::new(CrdConfigMapper::map_http_config(http_config.clone()))),
            Provider::Vault(vault_config) => Box::new(VaultConfigStore::new(CrdConfigMapper::map_vault_config(vault_config.clone()))),
        }
    }
}

// Define HTTP-specific configuration
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HttpConfig {
    pub url: String,
    pub headers : Option<HashMap<String, String>>,
    pub query_params : Option<HashMap<String, String>>
}

// Define Vault-specific configuration
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultConfig {
    pub server: String,
}

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(group = "external-config.com", version="v1alpha1", kind = "ConfigurationStore", namespaced )]
#[kube(status = "ConfigurationSourceStatus")]
pub struct ConfigurationStoreSpec {
    pub provider: Provider,
}

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(group = "external-config.com", version="v1alpha1", kind = "ClusterConfigurationStore" )]
#[kube(status = "ConfigurationSourceStatus")]
pub struct ClusterConfigurationStoreSpec {
    pub provider: Provider,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationSourceStatus {
    pub last_synced: Option<String>,
}
impl Default for ConfigurationSourceStatus {
    fn default() -> Self {
        Self {
            last_synced: None,
        }
    }
}

pub struct ConfigStoreFetcherAdapter {
    client: Arc<Client>,
    provider: Provider
}

pub struct CrdConfigMapper {}

impl CrdConfigMapper {

    fn map_http_config(http_config: HttpConfig) -> HttpConfigStoreConnectionDetails {

        HttpConfigStoreConnectionDetails {
            url: http_config.url.clone(),
            headers: http_config.headers.unwrap_or(HashMap::new()),
            query_params: http_config.query_params.unwrap_or(HashMap::new()),
        }
    }
    fn map_vault_config(vault_config: VaultConfig) -> VaultConfigStoreConnectionDetails {
        VaultConfigStoreConnectionDetails {
            url: vault_config.server.clone(),
            headers: HashMap::new(),
            query_params: HashMap::new()
        }
    }
}
