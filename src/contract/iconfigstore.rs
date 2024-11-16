use std::collections::HashMap;
use async_trait::async_trait;
use crate::contract::lib::Error;

#[async_trait]
pub trait IConfigStore: Send + Sync {


    async fn get_config(
        &self,
        query_params: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>
    ) -> Result<String, Error>;
}

