use crate::contract::clients::{ICrdClient, K8sClient, K8sClientAware};
use crate::contract::lib::Error;
use crate::controller::v1alpha1::controller::{
    ClusterConfigurationStore, ConfigMapClaim, ConfigurationStore, SecretClaim,
};
use kube::api::{ListParams, ObjectList};
use kube::{Api, Client, Resource};
use std::sync::Arc;

pub struct CrdClient {
    client: Arc<Client>,
}

impl CrdClient {
    pub fn new(client: Arc<Client>) -> Self {
        CrdClient { client }
    }
}

impl K8sClientAware for CrdClient {
    fn client(&self) -> Arc<Client> {
        self.client.clone()
    }
}
impl K8sClient for CrdClient {}

impl ICrdClient<ConfigurationStore, ClusterConfigurationStore, ConfigMapClaim, SecretClaim>
    for CrdClient
{
    async fn get_config_stores(
        &self,
        params: &ListParams,
        namespace: &str,
    ) -> Result<ObjectList<ConfigurationStore>, Error> {
        Api::<ConfigurationStore>::namespaced((*self.client).clone(), namespace)
            .list(&params)
            .await
            .map_err(Error::KubeError)
    }
    async fn get_cluster_config_stores(
        &self,
        params: &ListParams,
    ) -> Result<ObjectList<ClusterConfigurationStore>, Error> {
        Api::<ClusterConfigurationStore>::all((*self.client).clone())
            .list(&params)
            .await
            .map_err(Error::KubeError)
    }
    async fn get_cluster_config_store(
        &self,
        name: &str,
    ) -> Result<ClusterConfigurationStore, Error> {
        let store = Api::<ClusterConfigurationStore>::all((*self.client).clone())
            .get(&name)
            .await
            .map_err(Error::KubeError)?;

        Ok(store)
    }
    async fn get_config_store(
        &self,
        name: &str,
        namespace: &str,
    ) -> Result<ConfigurationStore, Error> {
        let store = Api::<ConfigurationStore>::namespaced((*self.client).clone(), namespace)
            .get(&name)
            .await
            .map_err(Error::KubeError)?;

        Ok(store)
    }
    async fn get_config_map_claims(
        &self,
        params: &ListParams,
        namespace: &str,
    ) -> Result<ObjectList<ConfigMapClaim>, Error> {
        let cmc_api = Api::<ConfigMapClaim>::namespaced((*self.client).clone(), namespace);
        cmc_api.list(&params).await.map_err(Error::KubeError)
    }
    async fn get_config_map_claim(
        &self,
        name: &str,
        namespace: &str,
    ) -> Result<ConfigMapClaim, Error> {
        let store = Api::<ConfigMapClaim>::namespaced((*self.client).clone(), namespace)
            .get(&name)
            .await
            .map_err(Error::KubeError)?;

        Ok(store)
    }

    async fn get_secret_claims(
        &self,
        params: &ListParams,
        namespace: &str,
    ) -> Result<ObjectList<SecretClaim>, Error> {
        let cmc_api = Api::<SecretClaim>::namespaced((*self.client).clone(), namespace);
        cmc_api.list(&params).await.map_err(Error::KubeError)
    }
    async fn get_secret_claim(&self, name: &str, namespace: &str) -> Result<SecretClaim, Error> {
        let store = Api::<SecretClaim>::namespaced((*self.client).clone(), namespace)
            .get(&name)
            .await
            .map_err(Error::KubeError)?;

        Ok(store)
    }
}
