use std::collections::{BTreeMap, HashMap};
use std::env;
use std::sync::Arc;
use k8s_openapi::api::core::v1::{ConfigMapKeySelector, PodTemplateSpec, SecretKeySelector, Volume};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::{Api, ResourceExt};
use kube::api::{Patch, PatchParams, PostParams};
use kube::runtime::controller::Action;
use log::info;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::contract::clients::K8sClient;
use crate::contract::iconfigstore::IConfigStore;
use crate::contract::lib::Error;
use crate::controller::config_store::http_store::HttpConfigStore;
use crate::controller::config_store::vault_store::VaultConfigStore;
use crate::controller::utils::context::Data;
use crate::controller::v1alpha1::crd::configuration_store::{CrdConfigMapper, HttpConfig, Provider, VaultConfig};


#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStorePodTemplateSpec {
    pub metadata: Option<ConfigurationStoreObjectMeta>,
    pub spec: Option<ConfigurationStorePodSpec>,
}

impl ConfigurationStorePodTemplateSpec {
    fn to_spec(
        &self,
    ) -> PodTemplateSpec {
        PodTemplateSpec {
            metadata: self.metadata.clone().map(|m| k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: m.name,
                labels: m.labels.clone(),
                annotations: m.annotations.clone(),
                ..Default::default()
            }),
            spec: self.spec.clone().map(|s| k8s_openapi::api::core::v1::PodSpec {
                containers: s.containers.into_iter().map(|c| {
                    k8s_openapi::api::core::v1::Container {
                        name: c.name,
                        image: c.image,
                        env: c.env.map(|envs| {
                            envs.into_iter()
                                .map(|e| k8s_openapi::api::core::v1::EnvVar {
                                    name: e.name,
                                    value: e.value,
                                    value_from: e.value_from.map(|v| k8s_openapi::api::core::v1::EnvVarSource {
                                        secret_key_ref: v.secret_key_ref.map(|x|x.to_secret_key_spec()),
                                        config_map_key_ref: v.config_map_key_ref.map(|x|x.to_config_map_key_spec()),
                                        ..Default::default()
                                    }),
                                })
                                .collect()
                        }),
                        ..Default::default()
                    }
                }).collect(),
                // volumes: s.volumes,
                // affinity: s.affinity,
                // tolerations: s.tolerations,
                ..Default::default()
            }),
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreObjectMeta {
    pub name: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
    pub annotations: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStorePodSpec {
    pub containers: Vec<ConfigurationStoreContainer>,
    pub volumes: Option<Vec<ConfigurationStoreVolume>>,
    // pub affinity: Option<ConfigurationStoreAffinity>,
    pub tolerations: Option<Vec<ConfigurationStoreToleration>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreContainer {
    pub name: String,
    pub image: Option<String>,
    pub env: Option<Vec<ConfigurationStoreEnvVar>>,
    pub volume_mounts: Option<Vec<ConfigurationStoreVolumeMount>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreEnvVar {
    pub name: String,
    pub value: Option<String>, // Direct value
    pub value_from: Option<ConfigurationStoreEnvVarSource>, // Source reference
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreEnvVarSource {
    pub secret_key_ref: Option<ConfigurationStoreKeySelector>,
    pub config_map_key_ref: Option<ConfigurationStoreKeySelector>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreKeySelector {
    pub name: String,
    pub key: String,
}

impl ConfigurationStoreKeySelector {
    pub fn to_secret_key_spec(&self ) -> SecretKeySelector {
        SecretKeySelector{
            key: self.key.clone(),
            name: self.name.clone(),
            optional: None
        }
    }
    pub fn to_config_map_key_spec(&self ) -> ConfigMapKeySelector {
        ConfigMapKeySelector{
            key: self.key.clone(),
            name: self.name.clone(),
            optional: None
        }
    }
}


#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreVolume {
    pub name: String,
    pub empty_dir: Option<ConfigurationStoreEmptyDirVolumeSource>,
    pub config_map: Option<ConfigurationStoreConfigMapVolumeSource>,
    pub secret: Option<ConfigurationStoreSecretVolumeSource>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreEmptyDirVolumeSource {
    pub medium: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreConfigMapVolumeSource {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreSecretVolumeSource {
    pub secret_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreVolumeMount {
    pub name: String,
    pub mount_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreAffinity {
    // Define affinity-related fields as needed
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreToleration {
    pub key: Option<String>,
    pub operator: Option<String>,
    pub value: Option<String>,
    pub effect: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreServiceSpec {
    pub metadata: Option<ConfigurationStoreObjectMeta>,
    pub spec: Option<ConfigurationStoreServiceSpecDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreServiceSpecDetails {
    pub ports: Vec<ConfigurationStoreServicePort>, // List of ports
    pub selector: Option<BTreeMap<String, String>>, // Label selector for the Service
    pub type_: Option<String>, // Service type (ClusterIP, NodePort, LoadBalancer, etc.)
    pub session_affinity: Option<String>, // Session affinity (None, ClientIP, etc.)
}

impl ConfigurationStoreServiceSpecDetails {
    fn to_spec(
        &self,
    ) -> k8s_openapi::api::core::v1::ServiceSpec {
        k8s_openapi::api::core::v1::ServiceSpec {
            ports: Some(
                self
                    .ports
                    .clone()
                    .into_iter()
                    .map(|p| k8s_openapi::api::core::v1::ServicePort {
                        name: p.name,
                        port: p.port,
                        target_port: p.target_port.map(|x|IntOrString::Int(x)),
                        protocol: Some(p.protocol.unwrap_or_else(|| "TCP".to_string())),
                        ..Default::default()
                    })
                    .collect(),
            ),
            selector: self.selector.clone().take(),
            type_: self.type_.clone().take(),
            session_affinity: self.session_affinity.clone().take(),
            ..Default::default()
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationStoreServicePort {
    pub name: Option<String>,
    pub port: i32,
    pub target_port: Option<i32>,
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DeployAs {
    Deployment(DeployAsDeployment),
    StatefulSet(DeployAsStatefulSet),
    DaemonSet(DeployAsDaemonSet),
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct DeployAsDeployment {
    replicas: Option<i32>,
    service: ConfigurationStoreServiceSpec,
    pod_template: Option<ConfigurationStorePodTemplateSpec>,
    // pub affinity: Option<AffinitySpec>,
}
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct DeployAsStatefulSet {
    replicas: Option<i32>,
    service: ConfigurationStoreServiceSpec,
    pod_template: Option<ConfigurationStorePodTemplateSpec>,

}
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct DeployAsDaemonSet {
    service: ConfigurationStoreServiceSpec,
    pub pod_template: Option<ConfigurationStorePodTemplateSpec>,
}



struct ConfigStoreDeploymentReconcile {}

impl ConfigStoreDeploymentReconcile {
    pub async fn workload_exists(&self, ctx: Arc<Data>, name: &str) -> Result<bool, Error> {
        let namespace = env::var("KUBERNETES_NAMESPACE").unwrap_or_else(|_| "default".to_string()).as_str();

        match &self {
            DeployAs::Deployment(config) => {
                ctx.api_client.get_deployment(name, namespace).await?;
                Ok(true)
            },
            DeployAs::StatefulSet(vault_config) => {
                ctx.api_client.get_stateful_set(name, namespace).await?;
                Ok(true)
            },
            DeployAs::DaemonSet(vault_config) => {
                ctx.api_client.get_daemon_set(name, namespace).await?;
                Ok(true)
            }
        }

    }
    pub async fn reconcile_workload(&self, ctx: Arc<Data>) -> Result<(), Error> {
        let exists = self.workload_exists()
        //  match &self {
        //     DeployAs::Deployment(http_config) => {
        //         ctx.api_client.create_deployment().await?;
        //         return Ok(());
        //     },
        //     DeployAs::StatefulSet(vault_config) => {
        //         ctx.api_client.create_stateful_set().await?;
        //         return Ok(());
        //     },
        //     DeployAs::DaemonSet(vault_config) => {
        //         ctx.api_client.create_daemon_set().await?;
        //         return Ok(());
        //     }
        // }

        return Ok(());
    }

    pub async fn reconcile_service(&self, ctx: Arc<Data>) -> Result<(), Error> {
        return Ok(());
    }

    pub async fn reconcile_network_policy(&self, ctx: Arc<Data>) -> Result<(), Error> {
        return Ok(());
    }

    async fn reconcile(&self, ctx: Arc<Data>) -> crate::contract::lib::Result<Action> {
        let client = ctx.client.clone();
        let namespace = <Self as ResourceExt>::namespace(self).unwrap();
        let name = self.name_any();

        let resources: Api<TargetType> = Api::namespaced((*client).clone(), &namespace);
        info!("Reconciling resource: {} in namespace: {}", name, namespace);
        let target = self.get_target();

        match self.workload_exists(&target.name, ctx.clone()).await {
            Ok(existing) => {
                let desired_resource = self.create_resource_spec(ctx.clone()).await?;

                if existing_resource.get_data() != desired_resource.get_data() {
                    let patch = Patch::Apply(json!(&desired_resource));
                    let params = PatchParams::apply("configmap-claim-controller").force();
                    resources.patch(&target.name, &params, &patch).await.map_err(Error::KubeError)?;
                }
            }
            Err(kube::Error::Api(ref e)) if e.code == 404 => {
                let new_resource = self.create_resource_spec(ctx.clone()).await?;
                resources.create(&PostParams::default(), &new_resource).await.map_err(Error::KubeError)?;
            }
            Err(e) => return Err(Error::KubeError(e)),
        }

        Ok(Action::requeue(self.get_refresh_interval()))
    }
}