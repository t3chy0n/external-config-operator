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
use controller::utils::context::Data;
use crate::controller::leader_election::leader_election::LeaderElection;
use crate::controller::utils::signals::notify_cancellation_token;
use crate::controller::v1alpha1;
use crate::controller::v1alpha1::crd_client::CrdClient;

mod controller;
mod contract;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //TODO: Add configurable log level
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel::<()>();

    let token = Arc::new(CancellationToken::new());
    notify_cancellation_token(&token,  shutdown_recv);


    let client = Client::try_default().await.expect("failed to create kube Client");
    let c = Arc::new(client);


    let data = Data {
        client: c.clone(),
        v1alpha1: Arc::new(v1alpha1::crd_client::CrdClient::new(c.clone())),
        api_client: Arc::new(v1alpha1::crd_client::CrdClient::new(c.clone())),
    };

    let leader_elector = Arc::new(
        LeaderElection::new(token.clone(), Arc::new(data.clone()))
    );


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

    //Wait for a moment in case close signal was passed.
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    tokio::select! {
        _ = interval.tick() => {
            join![
                v1alpha1::controller::run(data.clone()),
            ];
        },
        _ = token.cancelled() => {
            info!("Shutdown signal received, before controllers started. Closing...");
        },
    }

    Ok(())


}
