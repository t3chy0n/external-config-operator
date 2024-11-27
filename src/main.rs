use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use futures::join;
use kube::{Api, Client};
use tracing::{error, info, Level};
use tokio::signal::unix::{signal, SignalKind};

use futures::stream::StreamExt;
use tokio::signal;
use tokio::sync::{mpsc, Notify};
use tokio_util::sync::CancellationToken;
use controller::utils::context::Context;
use crate::controller::controller_data::State;
use crate::controller::leader_election::leader_election::LeaderElection;
use crate::controller::utils::signals::notify_cancellation_token;
use crate::controller::v1alpha1;
use crate::controller::v1alpha1::crd_client::CrdClient;
use crate::observability::metrics_server::run_metrics_server;
use crate::observability::telemetry;

mod controller;
mod contract;
mod observability;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    telemetry::init().await;

    let client = Client::try_default().await.expect("failed to create kube Client");
    let c = Arc::new(client);

    let state = State::default();
    let context = state.to_context(c);

    let server_task = run_metrics_server(state)?;
    tokio::spawn(server_task);

    //TODO: Add configurable log level
    // tracing_subscriber::fmt()
    //     .with_max_level(Level::INFO)
    //     .init();

    let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel::<()>();

    let token = Arc::new(CancellationToken::new());
    notify_cancellation_token(&token,  shutdown_recv);


    let leader_elector = Arc::new(
        LeaderElection::new(token.clone(), context.clone())
    );

    if leader_elector.enabled() {
        let lease = leader_elector.claim_leadership_loop().await;

        match lease {
            Ok(Some(lease)) => {
                tokio::spawn(async move {
                    leader_elector.refresh_leadership_loop().await;
                });
            }
            Ok(None) => {
                info!("No k8s env variables set. Startus controller...");
            },
            Err(e) => {
                info!("There was some error when trying to claim lease. Closing... {:?}", e);
                token.cancel()
            }
        }
    }

    //Wait for a moment in case close signal was passed.
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    tokio::select! {
        _ = interval.tick() => {
            join![
                v1alpha1::controller::run((*context).clone())
            ];
        },
        _ = token.cancelled() => {
            info!("Shutdown signal received, before controllers started. Closing...");
        },
    }

    Ok(())


}
