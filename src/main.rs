use std::sync::Arc;
use futures::join;
use kube::{Api, Client};
use tracing::{error, info, Level};

use futures::stream::StreamExt;
use controller::utils::context::Data;

mod controller;
mod contract;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let client = Client::try_default().await.expect("failed to create kube Client");
    let c = Arc::new(client);


    let data = Data { client: c};

    join![
        controller::v1alpha1::controller::run(data.clone())
    ];

    // Controller::new(cm_api, Config::default())
    //     .shutdown_on_signal()
    //     .run(reconcile, error_policy, Arc::new(data))
    //     .for_each(|res| async move {
    //         match res {
    //             Ok(o) => info!("Reconciled {:?}", o),
    //             Err(e) => error!("Reconcile failed: {:?}", e),
    //         }
    //     })
    //     .await;

    Ok(())


}
