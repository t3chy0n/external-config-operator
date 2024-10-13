use async_trait::async_trait;
use crate::contract::lib::Error;

#[async_trait]
pub trait IConfigStore: Send + Sync {


    async fn get_config(&self, key: String) -> Result<String, Error>;
}

