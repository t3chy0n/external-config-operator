use std::sync::Arc;
use futures::join;
use kube::{Api, Client};
use tracing::{error, info, Level};

use futures::stream::StreamExt;
use tokio::sync::Notify;
use controller::utils::context::Data;
use crate::controller::leader_election::leader_election::LeaderElection;

mod controller;
mod contract;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let client = Client::try_default().await.expect("failed to create kube Client");
    let c = Arc::new(client);
    let leader_elector = LeaderElection::new(c.clone());


    let data = Data {
        client: c.clone(),
    };

    let lease = leader_elector.claim_leadership_loop().await;
    let lease_refresh_task = tokio::task::spawn(async move {
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
    });

    join![
        controller::v1alpha1::controller::run(data.clone()),
        lease_refresh_task
    ];

    Ok(())


}
