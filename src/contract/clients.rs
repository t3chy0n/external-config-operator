use std::sync::Arc;
use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use kube::api::{ListParams, ObjectList};
use kube::{Api, Client, Resource};
use crate::contract::ireconcilable::IReconcilable;
use crate::contract::lib::Error;

pub trait K8sClientAware {
    fn client(&self) -> Arc<Client>;
}


pub trait ICrdClient<
    CS: Resource + Clone,
    CCS: Resource + Clone,
    CMC: Resource + IReconcilable + Clone,
    SC: Resource + IReconcilable + Clone
>: K8sClientAware {
    async fn get_config_map(&self, name: &str, namespace: &str) -> Result<ConfigMap, Error> {
        let client = self.client().as_ref().clone();
        let store = Api::<ConfigMap>::namespaced(client, namespace)
            .get(&name).await
            .map_err(Error::KubeError)?;

        Ok(store)
    }
    async fn get_secret(&self, name: &str, namespace: &str) -> Result<Secret, Error> {
        let client = self.client().as_ref().clone();
        let store = Api::<Secret>::namespaced(client, namespace)
            .get(&name).await
            .map_err(Error::KubeError)?;

        Ok(store)
    }
    async fn get_config_stores(&self, params: &ListParams, namespace: &str) -> Result<ObjectList<CS>, Error>;
    async fn get_cluster_config_stores(&self, params: &ListParams) -> Result<ObjectList<CCS>, Error>;
    async fn get_cluster_config_store(&self, name: &str) -> Result<CCS, Error>;
    async fn get_config_store(&self, name: &str, namespace: &str) -> Result<CS, Error>;
    async fn get_config_map_claims(&self, params: &ListParams, namespace: &str) -> Result<ObjectList<CMC>, Error>;
    async fn get_config_map_claim(&self, name: &str, namespace: &str) -> Result<CMC, Error>;

    async fn get_secret_claims(&self, params: &ListParams, namespace: &str) -> Result<ObjectList<SC>, Error>;
    async fn get_secret_claim(&self, name: &str, namespace: &str) -> Result<SC, Error>;
}