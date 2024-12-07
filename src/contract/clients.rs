use crate::contract::ireconcilable::IReconcilable;
use crate::contract::lib::Error;
use chrono::{Duration, Utc};
use either::Either;
use k8s_openapi::api::coordination::v1::{Lease, LeaseSpec};
use k8s_openapi::api::core::v1::{ConfigMap, Pod, Secret};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, OwnerReference};
use kube::api::{DeleteParams, ListParams, ObjectList, PostParams};
use kube::core::Status;
use kube::{Api, Client, Resource};
use log::warn;
use std::env;
use std::sync::Arc;
use tracing::info;

pub trait K8sClientAware {
    fn client(&self) -> Arc<Client>;
}

pub static CONTROLLER_LEASE_NAME: &str = "external-config-operator-leader-election";

pub trait K8sClient: K8sClientAware + Send + Sync {
    async fn get_config_map(&self, name: &str, namespace: &str) -> Result<ConfigMap, Error> {
        let client = self.client().as_ref().clone();
        let store = Api::<ConfigMap>::namespaced(client, namespace)
            .get(&name)
            .await
            .map_err(Error::KubeError)?;

        Ok(store)
    }
    async fn get_secret(&self, name: &str, namespace: &str) -> Result<Secret, Error> {
        let client = self.client().as_ref().clone();
        let store = Api::<Secret>::namespaced(client, namespace)
            .get(&name)
            .await
            .map_err(Error::KubeError)?;

        Ok(store)
    }
    async fn get_lease(&self, name: &str, namespace: &str) -> Result<Lease, Error> {
        let client = self.client().as_ref().clone();
        let lease = Api::<Lease>::namespaced(client, namespace)
            .get(&name)
            .await
            .map_err(Error::KubeError)?;

        Ok(lease)
    }
    async fn create_lease(
        &self,
        params: &PostParams,
        lease: &Lease,
        namespace: &str,
    ) -> Result<Lease, Error> {
        let client = self.client().as_ref().clone();
        let lease = Api::<Lease>::namespaced(client, namespace)
            .create(params, lease)
            .await
            .map_err(Error::KubeError)?;

        Ok(lease)
    }
    async fn replace_lease(
        &self,
        name: &str,
        params: &PostParams,
        lease: &Lease,
        namespace: &str,
    ) -> Result<Lease, Error> {
        let client = self.client().as_ref().clone();
        let lease = Api::<Lease>::namespaced(client, namespace)
            .replace(name, params, lease)
            .await
            .map_err(Error::KubeError)?;

        Ok(lease)
    }
    async fn delete_lease(
        &self,
        name: &str,
        params: &DeleteParams,
        namespace: &str,
    ) -> Result<Either<Lease, Status>, Error> {
        let client = self.client().as_ref().clone();
        let lease = Api::<Lease>::namespaced(client, namespace)
            .delete(name, params)
            .await
            .map_err(Error::KubeError)?;

        Ok(lease)
    }

    async fn get_pod(&self, name: &str, namespace: &str) -> Result<Pod, Error> {
        let client = self.client().as_ref().clone();
        let pod = Api::<Pod>::namespaced(client, namespace)
            .get(&name)
            .await
            .map_err(Error::KubeError)?;

        Ok(pod)
    }
    async fn try_create_lease_for_current_pod(&self) -> Result<Lease, Error> {
        let namespace = env::var("KUBERNETES_NAMESPACE").unwrap_or_else(|_| "default".to_string());
        let pod_name = env::var("KUBERNETES_POD_NAME")
            .expect("KUBERNETES_POD_NAME variable should be set, when leader election is enabled");

        let pod = self.get_pod(&pod_name, namespace.as_str()).await?;
        let pod_uid = pod.metadata.uid.clone().expect("Pod UID should be present");

        let mut lease = Lease {
            metadata: kube::api::ObjectMeta {
                name: Some(CONTROLLER_LEASE_NAME.to_string()),
                owner_references: Some(vec![OwnerReference {
                    api_version: "v1".to_string(),
                    kind: "Pod".to_string(),
                    name: pod_name.to_string(),
                    uid: pod_uid,
                    controller: Some(true),
                    block_owner_deletion: Some(false),
                }]),
                ..Default::default()
            },
            spec: Some(LeaseSpec {
                holder_identity: Some(pod_name.clone()),
                acquire_time: Some(MicroTime(Utc::now())),
                renew_time: None,
                lease_duration_seconds: Some(20),
                lease_transitions: None,
                preferred_holder: None,
                strategy: None,
            }),
        };
        self.create_lease(&PostParams::default(), &lease, namespace.as_str())
            .await;

        let lease = self
            .get_lease(CONTROLLER_LEASE_NAME, namespace.as_str())
            .await?;
        let holder = lease
            .spec
            .as_ref()
            .and_then(|spec| spec.holder_identity.as_ref());
        let last_renewal = lease
            .spec
            .as_ref()
            .and_then(|spec| spec.renew_time.as_ref());

        if let Some(last_renewal) = last_renewal {
            // Parse MicroTime into a chrono::DateTime<Utc>
            let renewal_time = last_renewal.0;
            let now = Utc::now();

            // Check if the duration since last renewal is greater than 2 minutes
            if now.signed_duration_since(renewal_time) > Duration::minutes(2) {
                // Force delete the lease
                self.delete_lease(
                    &lease.metadata.name.clone().unwrap(),
                    &kube::api::DeleteParams::default(),
                    &namespace,
                )
                .await?;
                println!(
                    "Lease {} was deleted due to expired renew time",
                    CONTROLLER_LEASE_NAME
                );
            }
        }
        if holder == Some(&pod_name) {
            info!("Acquired leadership with lease: {}", CONTROLLER_LEASE_NAME);
            Ok(lease)
        } else {
            Err(Error::LeaseHeldByAnotherPod())
        }
    }
}

pub trait ICrdClient<
    CS: Resource + Clone,
    CCS: Resource + Clone,
    CMC: Resource + IReconcilable + Clone,
    SC: Resource + IReconcilable + Clone,
>: K8sClientAware + K8sClient
{
    async fn get_config_stores(
        &self,
        params: &ListParams,
        namespace: &str,
    ) -> Result<ObjectList<CS>, Error>;
    async fn get_cluster_config_stores(
        &self,
        params: &ListParams,
    ) -> Result<ObjectList<CCS>, Error>;
    async fn get_cluster_config_store(&self, name: &str) -> Result<CCS, Error>;
    async fn get_config_store(&self, name: &str, namespace: &str) -> Result<CS, Error>;
    async fn get_config_map_claims(
        &self,
        params: &ListParams,
        namespace: &str,
    ) -> Result<ObjectList<CMC>, Error>;
    async fn get_config_map_claim(&self, name: &str, namespace: &str) -> Result<CMC, Error>;

    async fn get_secret_claims(
        &self,
        params: &ListParams,
        namespace: &str,
    ) -> Result<ObjectList<SC>, Error>;
    async fn get_secret_claim(&self, name: &str, namespace: &str) -> Result<SC, Error>;
}
