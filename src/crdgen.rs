use crate::controller::v1alpha1::crd::claim::{ConfigMapClaim, SecretClaim};
use crate::controller::v1alpha1::crd::configuration_store::{
    ClusterConfigurationStore, ConfigurationStore,
};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::PostParams;
use kube::{Api, Client, CustomResourceExt, ResourceExt};

mod contract;
mod controller;
mod observability;
fn main() {
    let crds = generate_crd_manifests();

    crds.iter().enumerate().for_each(|(i, crd)| {
        let crd_yaml = serde_yaml::to_string(&crd).unwrap();

        println!("{crd_yaml}");
        // Print "---" only if it's not the last element
        if i < crds.len() - 1 {
            println!("---");
        }
    })
}

fn generate_crd_manifests() -> Vec<CustomResourceDefinition> {
    let t = ConfigMapClaim::crd();

    let crds: Vec<CustomResourceDefinition> = vec![
        ConfigurationStore::crd(),
        ClusterConfigurationStore::crd(),
        ConfigMapClaim::crd(),
        SecretClaim::crd(),
    ];

    crds
}

async fn apply_crd(
    client: &Client,
    crd: &CustomResourceDefinition,
) -> Result<(), Box<dyn std::error::Error>> {
    let crd_yaml = serde_yaml::to_string(&crd)?;
    println!("{crd_yaml}");
    println!("---");

    // Define the Kubernetes API endpoint for CRDs
    let crd_api: Api<CustomResourceDefinition> = Api::all(client.clone());

    // Apply the CRD
    match crd_api.create(&PostParams::default(), crd).await {
        Ok(created) => {
            println!("Applied CRD: {}", created.name_any());
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to apply CRD: {:?}", e);
            Err(e.into())
        }
    }
}

async fn apply_all_crds(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let crds = generate_crd_manifests();
    for crd in crds.iter() {
        apply_crd(&client, crd).await?;
    }

    Ok(())
}
