use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::NamespaceResourceScope;
use kube::{
    api::{Api, ListParams, Patch, PatchParams, ResourceExt, DynamicObject, GroupVersionKind, PostParams},
    client::Client,
    runtime::{
        controller::{Action, Controller},
        events::{Event, EventType, Recorder, Reporter},
        finalizer::{finalizer, Event as Finalizer},
        watcher::Config,
    },
    CustomResource, Resource,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use futures::stream::StreamExt;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::ApiResource;
use serde_yaml::Value;
use tracing::{error, info};
use crate::contract::ireconcilable::{ControllerReconcilableTargetTypeBounds, IReconcilable};
use crate::contract::lib::Error;
use crate::controller::utils::context::Data;
use crate::contract::lib::Result;
use crate::controller::v1alpha1::controller::crds;
use crate::controller::v1alpha1::crd::claim::{ConfigMapClaim, SecretClaim};
use crate::controller::v1alpha1::crd::configuration_store::{ClusterConfigurationStore, ConfigurationStore};

// #[instrument(skip(ctx, doc), fields(trace_id))]
pub static DOCUMENT_FINALIZER: &str = "test.io/documents.kube3.rs";

pub async fn reconcile<T >(resource: Arc<T>, ctx: Arc<Data>) -> Result<Action>
where
    T: ControllerReconcilableTargetTypeBounds
{
    // let trace_id = telemetry::get_trace_id();
    // Span::current().record("trace_id", &field::display(&trace_id));
    // let _timer = ctx.metrics.reconcile.count_and_measure(&trace_id);
    // ctx.diagnostics.write().await.last_event = Utc::now();
    let ns = resource.namespace().unwrap(); // doc is namespace scoped
    let resources: Api<T> = Api::namespaced((*ctx.client).clone(), &ns);

    info!("Reconciling \"{}\" in {}", resource.name_any(), ns);
    finalizer(&resources, DOCUMENT_FINALIZER, resource, |event| async {
        match event {
            Finalizer::Apply(doc) => doc.reconcile(ctx.clone()).await,
            Finalizer::Cleanup(doc) => (*doc).clone().cleanup(ctx.clone()).await,
        }
    })
    .await
    .map_err(|e| Error::FinalizerError(Box::new(e)))
}

pub fn error_policy<T>(resource: Arc<T>, error: &Error, _ctx: Arc<Data>) -> Action
where
    T: ControllerReconcilableTargetTypeBounds
{

    error!("Error reconciling: {:?}", error);
    Action::requeue(Duration::from_secs(60))


}

pub async fn run<T: Resource + IReconcilable>(data: Data)
where
    T: ControllerReconcilableTargetTypeBounds
{
    let api: Api<T> = Api::all((*data.client).clone());

    if let Err(e) = api.list(&ListParams::default().limit(1)).await {
        error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    Controller::new(api, Config::default())
        .shutdown_on_signal()
        .run(reconcile::<T>, error_policy::<T>, Arc::new(data))
        .for_each(|res| async move {
            match res {
                Ok(o) => info!("Reconciled {:#?}", o),
                Err(e) => error!("Reconcile failed: {:?}", e),
            }
        })
        .await;

}




async fn apply_crd(client: Arc<Client>, crd: &CustomResourceDefinition) -> std::result::Result<(), Box<dyn std::error::Error>> {

    // Define the Kubernetes API endpoint for CRDs
    let crd_api: Api<CustomResourceDefinition> = Api::all((*client).clone());

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



pub async fn apply_all_crds(client: Arc<Client>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let crds:Vec<CustomResourceDefinition> = crate::controller::v1alpha1::controller::crds();
        // .extend(); TODO: For new versions extend this
    for crd in crds.iter() {
        apply_crd(client.clone(), crd).await?;
    }

    Ok(())
}

fn is_namespaced(kind: &str) -> bool {
    // Specify cluster-scoped resource kinds here
    match kind {
        "Node" | "Namespace" | "ClusterRole" | "ClusterRoleBinding" | "PersistentVolume" | "ClusterConfigurationStore" => false,
        _ => true,
    }
}
/// Utility function to deploy a CRD from a YAML file
pub async fn apply_from_yaml(client: Arc<Client>, manifest_yaml: &str) -> Result<(), Box<dyn std::error::Error>> {

    // Parse the YAML into a serde_yaml::Value to read its metadata
    let manifest: Value = serde_yaml::from_str(&manifest_yaml)?;

    // Extract necessary fields for GroupVersionKind
    let api_version = manifest.get("apiVersion").and_then(|v| v.as_str()).ok_or("Missing apiVersion")?.to_string();
    let kind = manifest.get("kind").and_then(|v| v.as_str()).ok_or("Missing kind")?.to_string();
    let metadata = manifest.get("metadata").ok_or("Missing metadata")?;
    let namespace = metadata.get("namespace").and_then(|v| v.as_str()).unwrap_or("default").to_string();
    let namespace = if is_namespaced(&kind) {
        metadata.get("namespace").and_then(|v| v.as_str()).unwrap_or("default").to_string()
    } else {
        String::new() // Empty string for cluster-scoped resources
    };
    // Create a DynamicObject with GroupVersionKind
    // Split apiVersion into group and version
    let parts: Vec<&str> = api_version.split('/').collect();
    let (group, version) = if parts.len() == 2 {
        (parts[0], parts[1])
    } else {
        ("", parts[0]) // Core API group has no group part, only version
    };


    // Create a GroupVersionKind with group, version, and kind
    let gvk = GroupVersionKind::gvk(group, version, kind.as_str());
    let api_resource = ApiResource::from_gvk(&gvk);
    let dynamic_api: Api<DynamicObject> = if namespace.is_empty() {
        Api::all_with((*client).clone(), &api_resource) // Cluster-scoped
    } else {
        Api::namespaced_with((*client).clone(), &namespace, &api_resource) // Namespaced
    };

    // Convert manifest into DynamicObject and apply it
    let dynamic_object: DynamicObject = serde_yaml::from_value(manifest)?;

    match dynamic_api.create(&PostParams::default(), &dynamic_object).await {
        Ok(created) => {
            println!("Successfully deployed {}: {}", kind, created.name_any());
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to deploy {}: {:?}", kind, e);
            Err(e.into())
        }
    }
}