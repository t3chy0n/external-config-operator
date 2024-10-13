use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::CustomResourceExt;
use crate::controller::v1alpha1::crd::configuration_store::{ConfigurationStore, ClusterConfigurationStore};
use crate::controller::v1alpha1::crd::claim::{ConfigMapClaim, SecretClaim};

mod controller;
mod contract;
fn main() {
    let t = ConfigMapClaim::crd();

    let crds: Vec<CustomResourceDefinition> = vec![
        ConfigurationStore::crd(),
        ClusterConfigurationStore::crd(),
        ConfigMapClaim::crd(),
        SecretClaim::crd(),
    ];

    crds.iter().for_each(|crd| {
        let crd_yaml = serde_yaml::to_string(&crd).unwrap();

        println!("{crd_yaml}");
        println!("---");
    })

}