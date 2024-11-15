use std::env;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use chrono::Utc;
use k8s_openapi::api::coordination::v1::{Lease, LeaseSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, OwnerReference};
use kube::{Api, Client};
use kube::api::{Patch, PatchParams, PostParams};
use log::{debug, error, info, warn};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use crate::contract::clients::{K8sClient, CONTROLLER_LEASE_NAME};
use crate::controller::utils::context::Data;
use crate::contract::lib::Error;

pub struct LeaderElection {
    cancel_token: Arc<CancellationToken>,
    ctx: Arc<Data>,
}


impl LeaderElection {
    pub fn new(cancel_token: Arc<CancellationToken>, ctx: Arc<Data>) -> Self {
        let namespace = env::var("KUBERNETES_NAMESPACE").unwrap_or_else(|_| "default".to_string());

        LeaderElection {
            cancel_token,
            ctx,
        }
    }

    async fn run_cancellable<F, Fut, T>(&self, f: F) -> Result<T, Error>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, Error>>,
    {
        tokio::select! {
            result = f() => result,
            _ = self.cancel_token.cancelled() => {
                info!("Shutdown signal received. Stopping...");
                Err(Error::Cancelled)
            },
        }
    }


    pub async fn claim_leadership_loop(&self) -> Result<Option<Lease>, Error> {
        info!("Trying to acquire lease...");

       self.run_cancellable(|| async move {
            loop {
                let result =  self.ctx.api_client.try_create_lease_for_current_pod().await;
                match result {
                    Ok(lease) => return Ok(Some(lease)),
                    Err(e) =>{
                        sleep(Duration::from_secs(10));
                        debug!("Error when acquiring lease... {:?}", e);
                        continue
                    }
                }
            }
        }).await


    }
    pub async fn refresh_leadership_loop(&self) {
        let mut refresh_interval = tokio::time::interval(Duration::from_secs(10));
        let namespace = env::var("KUBERNETES_NAMESPACE").unwrap_or_else(|_| "default".to_string());

        let _ = self.run_cancellable(|| async move {
            loop {
                refresh_interval.tick().await;
                let mut lease = match self.ctx.api_client.get_lease(&CONTROLLER_LEASE_NAME, &namespace.as_str()).await {
                    Ok(lease) => lease,
                    Err(e) => {
                            error!("Failed to get lease on refresh attempt: {:?}", e);
                            break;
                        }
                    };

                let mut lease_spec = lease.spec.as_mut().unwrap();
                lease_spec.renew_time = Some(MicroTime(Utc::now()));

                match self.ctx.api_client.replace_lease(&CONTROLLER_LEASE_NAME, &PostParams::default(), &lease, namespace.as_str()).await {
                    Ok(_) => {
                        info!("Successfully renewed lease: {}", CONTROLLER_LEASE_NAME)
                    },
                    Err(e) => {
                        error!("Failed to renew lease: {:?}", e);
                        break;
                    }
                }

            }
            Ok(())
        }).await;

        // Optionally, you can attempt to release the lease here by deleting it or removing holder_identity
        // match self.ctx.api_client.delete_lease(&CONTROLLER_LEASE_NAME, &Default::default(), &namespace.as_str()).await {
        //     Ok(_) => info!("Lease released upon shutdown: {}", CONTROLLER_LEASE_NAME),
        //     Err(e) => warn!("Failed to release lease on shutdown: {:?}", e),
        // }

    }
}