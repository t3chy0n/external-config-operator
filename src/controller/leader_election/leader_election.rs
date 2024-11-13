use std::env;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use chrono::Utc;
use k8s_openapi::api::coordination::v1::{Lease, LeaseSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, OwnerReference};
use kube::{Api, Client};
use kube::api::{Patch, PatchParams, PostParams};
use kube::runtime::finalizer::Error;
use log::{error, info, warn};
use serde_json::json;

pub struct LeaderElection {
    client: Arc<Client>,
    lease_api: Api<Lease>,
    pod_api: Api<k8s_openapi::api::core::v1::Pod>,
}

static CONTROLLER_LEASE_NAME: &str = "external-config-operator-leader-election";

impl LeaderElection {
    pub fn new(client: Arc<Client>) -> Self {
        let namespace = env::var("KUBERNETES_NAMESPACE").unwrap_or_else(|_| "default".to_string());
        let lease_api: Api<Lease> = Api::namespaced((*client).clone(), namespace.as_str());
        let pod_api: Api<k8s_openapi::api::core::v1::Pod> = Api::namespaced((*client).clone(), &namespace);

        LeaderElection {
            client,
            lease_api,
            pod_api
        }
    }
    pub async fn try_claim_leadership(&self) -> Result<Option<Lease>, kube::Error> {
        let pod_identity = env::var("KUBERNETES_POD_NAME");

        match pod_identity {
            Ok(pod_name) => {

                let pod = self.pod_api.get(&pod_name).await?;
                let pod_uid = pod.metadata.uid.clone().expect("Pod UID should be present");

                let mut lease = Lease {
                    metadata: kube::api::ObjectMeta {
                        name: Some(CONTROLLER_LEASE_NAME.to_string()),
                        owner_references: Some(vec![OwnerReference {
                            api_version: "v1".to_string(),
                            kind: "Pod".to_string(),
                            name: pod_name.to_string(),
                            uid: pod_uid,
                            controller: Some(true),
                            block_owner_deletion: Some(false),
                        }]),
                        ..Default::default()
                    },
                    spec: Some(LeaseSpec {
                        holder_identity: Some(pod_name),
                        acquire_time: Some(MicroTime(Utc::now())),
                        renew_time: None,
                        lease_duration_seconds: Some(20),
                        lease_transitions: None,
                        preferred_holder: None,
                        strategy: None,

                    }),
                };

                // Try to acquire the lease (first instance to do so becomes the leader)
                match self.lease_api.create(&PostParams::default(), &lease).await {
                    Ok(lease) => {
                        match self.is_owner_of_lease().await {
                            Ok(is_owner) => {
                                if is_owner {
                                    info!("Acquired leadership with lease: {}", CONTROLLER_LEASE_NAME);
                                    return Ok(Some(lease))
                                }
                                panic!("Pod is not owner of a lease")

                            }
                            Err(e) => {
                                warn!("Failed to acquire lease (another controller might be the leader): {:?}", e);
                                Err(e)
                            }
                        }

                    }
                    Err(e) => {
                        warn!("Failed to acquire lease (another controller might be the leader): {:?}", e);
                        Err(e)
                    }
                }
            },
            Err(e) => {
                warn!("Failed to configure leader election (is KUBERNETES_POD_NAME and KUBERNETES_NAMESPACE env variables defined?): {:?}", e);
                Ok(None)
            }
        }

    }

    pub async fn claim_leadership_loop(&self) -> Result<Option<Lease>, kube::Error> {
        loop {
            let result = self.try_claim_leadership().await;
            match result {
                Ok(_) => return result,
                Err(_) =>{
                    sleep(Duration::from_secs(10));
                    continue

                }
            }
        }
    }

    async fn is_owner_of_lease(&self) -> Result<bool, kube::Error> {

        let pod_identity = env::var("KUBERNETES_POD_NAME").expect("KUBERNETES_POD_NAME variable should be set, when leader election is enabled");
        let patch = json!({
            "spec": {
                "holderIdentity": pod_identity,
                "renewTime": Utc::now()
            }
        });

        match self.lease_api
            .get(&CONTROLLER_LEASE_NAME)
            .await {
            Ok(lease) => {
                Ok(lease.spec.unwrap().holder_identity.unwrap() == pod_identity)
            },
            Err(e) => {
                Err(e)
            }
        }

    }

    pub async fn refresh_leadership_loop(&self, lease: Lease) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        let namespace = env::var("KUBERNETES_NAMESPACE").unwrap_or_else(|_| "default".to_string());
        let pod_identity = env::var("KUBERNETES_POD_NAME");

        let lease_api: Api<Lease> = Api::namespaced((*self.client).clone(), namespace.as_str());

        match pod_identity {
            Ok(pod_name) => {
                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            let mut lease = match lease_api.get(&CONTROLLER_LEASE_NAME).await {
                                Ok(lease) => lease,
                                Err(e) => {
                                        error!("Failed to get lease on refresh attempt: {:?}", e);
                                        break;
                                    }
                                };

                            let mut lease_spec = lease.spec.as_mut().unwrap();
                            lease_spec.renew_time = Some(MicroTime(Utc::now()));

                            match lease_api.replace(&CONTROLLER_LEASE_NAME, &PostParams::default(), &lease).await {
                                Ok(_) => {
                                    info!("Successfully renewed lease: {}", CONTROLLER_LEASE_NAME)

                                },
                                Err(e) => {
                                    error!("Failed to renew lease: {:?}", e);

                                    tokio::signal::ctrl_c().await;
                                    break;
                                }
                            }
                        },
                        _ = tokio::signal::ctrl_c() => {
                            info!("Shutdown signal received. Stopping lease refresh.");
                            break;
                        },
                    }
                }

                // Optionally, you can attempt to release the lease here by deleting it or removing holder_identity
                match lease_api.delete(&CONTROLLER_LEASE_NAME, &Default::default()).await {
                    Ok(_) => info!("Lease released upon shutdown: {}", CONTROLLER_LEASE_NAME),
                    Err(e) => warn!("Failed to release lease on shutdown: {:?}", e),
                }
            },
            Err(e) => {
                warn!("Failed to configure leader election (is KUBERNETES_POD_NAME and KUBERNETES_NAMESPACE env variables defined?): {:?}", e);
                return;
            }
        }

    }
}