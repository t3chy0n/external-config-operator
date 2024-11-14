use std::cell::OnceCell;
use std::sync::Arc;
use kube::Client;
use tokio::sync::Notify;
use crate::contract::clients::K8sClient;
use crate::controller::v1alpha1;

pub struct ConfigurationManager {

}

impl ConfigurationManager {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct DependencyContainer {
    config: OnceCell<ConfigurationManager>
}

impl DependencyContainer {
    pub fn new() -> Self {
        Self {
            config: OnceCell::new()
        }
    }
    fn create_configuration_manager(&self) -> ConfigurationManager {
        ConfigurationManager::new()
    }
    pub fn config_manager(&self) -> &ConfigurationManager {
        self.config.get_or_init(|| self.create_configuration_manager())
    }
}

#[derive(Clone)]
pub struct Data {
    pub client: Arc<Client>,
    pub v1alpha1: Arc<v1alpha1::crd_client::CrdClient>,
    pub api_client: Arc<v1alpha1::crd_client::CrdClient>
}