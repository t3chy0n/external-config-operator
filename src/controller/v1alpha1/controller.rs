use futures::join;
use kube::CustomResourceExt;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use crate::controller::utils::context::Data;
use crate::controller::controller::{run as startController};

pub use super::crd::claim::{ConfigMapClaim, SecretClaim};
pub use crate::controller::v1alpha1::crd::configuration_store::{ClusterConfigurationStore, ConfigurationStore};

pub async fn run(data: Data)  {
    join![
        startController::<ConfigMapClaim>(data.clone()),
        startController::<SecretClaim>(data.clone())
    ];

}

pub fn crds() -> Vec<CustomResourceDefinition> {
    let t = ConfigMapClaim::crd();

    let crds: Vec<CustomResourceDefinition> = vec![
        ConfigurationStore::crd(),
        ClusterConfigurationStore::crd(),
        ConfigMapClaim::crd(),
        SecretClaim::crd(),
    ];

    crds
}
