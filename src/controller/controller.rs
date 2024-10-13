use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::NamespaceResourceScope;
use kube::{
    api::{Api, ListParams, Patch, PatchParams, ResourceExt},
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
use tracing::{error, info};
use crate::contract::ireconcilable::IReconcilable;
use crate::contract::lib::Error;
use crate::controller::utils::context::Data;
use crate::contract::lib::Result;

// #[instrument(skip(ctx, doc), fields(trace_id))]
pub static DOCUMENT_FINALIZER: &str = "test.io/documents.kube3.rs";

async fn reconcile<T >(resource: Arc<T>, ctx: Arc<Data>) -> Result<Action>
where
    T: Resource<Scope = NamespaceResourceScope, DynamicType = ()> + ResourceExt + IReconcilable  + Send + Sync + Clone + Serialize + Debug + DeserializeOwned + 'static
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
    T: Resource<Scope = NamespaceResourceScope, DynamicType = ()> + ResourceExt + IReconcilable  + Send + Sync + Clone + Serialize + Debug + DeserializeOwned + 'static
{

    error!("Error reconciling: {:?}", error);

    Action::requeue(Duration::from_secs(60))


}

pub async fn run<T: Resource + IReconcilable>(data: Data)
where
    T: Resource<Scope = NamespaceResourceScope, DynamicType = ()> + ResourceExt + IReconcilable  + Send + Sync + Clone + Serialize + Debug + DeserializeOwned + 'static
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