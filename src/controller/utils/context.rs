use std::cell::OnceCell;
use std::sync::Arc;
use kube::Client;


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
}