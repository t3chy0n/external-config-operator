use std::sync::Arc;
use tokio::sync::RwLock;

use crate::controller::utils::context::Context;
use crate::controller::v1alpha1::crd_client::CrdClient;
use crate::observability::metrics::Metrics;
use chrono::{DateTime, Utc};
use kube::runtime::events::{Recorder, Reporter};
use kube::Client;
use serde::Serialize;
use crate::controller::utils::config::Config;

/// Diagnostics to be exposed by the web server
#[derive(Clone, Serialize)]
pub struct Diagnostics {
    #[serde(deserialize_with = "from_ts")]
    pub last_event: DateTime<Utc>,
    #[serde(skip)]
    pub reporter: Reporter,
}
impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            last_event: Utc::now(),
            reporter: "doc-controller".into(),
        }
    }
}
// impl Diagnostics {
//     fn recorder(&self, client: Client, doc: &Document) -> Recorder {
//         Recorder::new(client, self.reporter.clone(), doc.object_ref(&()))
//     }
// }

/// State shared between the controller and the web server
#[derive(Clone, Default)]
pub struct State {
    /// Diagnostics populated by the reconciler
    diagnostics: Arc<RwLock<Diagnostics>>,
    /// Metrics
    metrics: Arc<Metrics>,
}

/// State wrapper around the controller outputs for the web server
impl State {
    /// Metrics getter
    pub fn metrics(&self) -> String {
        let mut buffer = String::new();
        let registry = &*self.metrics.registry;
        prometheus_client::encoding::text::encode(&mut buffer, registry).unwrap();
        buffer
    }

    /// State getter
    pub async fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.read().await.clone()
    }

    // Create a Controller Context that can update State
    pub fn to_context(&self, client: Arc<Client>) -> Arc<Context> {
        Arc::new(Context {
            client: client.clone(),
            metrics: self.metrics.clone(),
            v1alpha1: Arc::new(CrdClient::new(client.clone())),
            api_client: Arc::new(CrdClient::new(client.clone())),
            // diagnostics: self.diagnostics.clone(),
        })
    }
}
