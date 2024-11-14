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

    let token = Arc::new(CancellationToken::new());
    let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel::<()>();


    let client = Client::try_default().await.expect("failed to create kube Client");
    let c = Arc::new(client);
    let leader_elector = LeaderElection::new(token.clone(), c.clone());


    let data = Data {
        client: c.clone(),
        v1alpha1: Arc::new(v1alpha1::crd_client::CrdClient::new(c)),
    };

    tokio::spawn({
        let token = token.clone();

        async move {
            //TODO: Unlikely it will run on windows node, but for that case its unsafe
            let mut sigterm = signal(SignalKind::terminate()).unwrap();

            tokio::select! {
                _ = signal::ctrl_c() => {
                   info!("Received ctrl+C, initiating graceful shutdown.");
                },
                _ = sigterm.recv() => {
                    info!("Received SIGTERM, initiating graceful shutdown.");
                },
                _ = shutdown_recv.recv() => {
                    info!("Received shutdown signal, initiating graceful shutdown.");
                }
            }


            token.cancel();
        }
    });

    let lease = leader_elector.claim_leadership_loop().await;
    let lease_refresh_task = tokio::task::spawn({
        let token = token.clone();
        async move {
            match lease {
                Ok(has_lease) => {
                    if let Some(lease) = has_lease {

                        leader_elector.refresh_leadership_loop(lease).await;

                    } else {
                        info!("No lease to keep refreshing");
                    }
                }
                Err(e) => {
                    info!("There was an error when fetching the lease {:?}", e);
                }
            }
        }
    });


    //Wait for a moment in case close signal was passed.
    let mut interval = tokio::time::interval(Duration::from_secs(3));
    tokio::select! {
        _ = interval.tick() => {
            join![
                controller::v1alpha1::controller::run(data.clone()),
                lease_refresh_task
            ];
        },
        _ = token.cancelled() => {
            info!("Shutdown signal received, before controllers started. Closing...");
        },
    }

    Ok(())


}
