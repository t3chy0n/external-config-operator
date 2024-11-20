use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::Arc;
use async_trait::async_trait;
use base64::Engine;
use tokio::time::Duration;
use chrono::format::{parse, ParseErrorKind};
use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use k8s_openapi::{ByteString, NamespaceResourceScope};
use kube::{Api, Client, CustomResource, Resource, ResourceExt};
use kube::api::{DeleteParams, Patch, PatchParams, PostParams};
use kube::core::object::HasSpec;
use kube::runtime::controller::Action;
use kube::runtime::events::{Event, EventType, Recorder, Reporter};
use kube::runtime::reflector::Lookup;
use schemars::JsonSchema;
use schemars::schema::{InstanceType, Metadata, Schema, SchemaObject};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use tracing::info;
use crate::contract::iconfigstore::IConfigStore;
use crate::contract::ireconcilable::{IReconcilable, ReconcilableTargetTypeBounds};
use crate::controller::utils::crd::{HasData, RefreshInterval};
use crate::controller::utils::context::Data;
use crate::controller::v1alpha1::crd::configuration_store::{ClusterConfigurationStore, ConfigStoreFetcherAdapter, ConfigurationStore, Provider};
use crate::contract::lib::{Error, Result};
use crate::controller::controller::DOCUMENT_FINALIZER;
use base64::engine::general_purpose::STANDARD;

use crate::controller::utils::file_format::{convert_to_format, convert_to_json, merge_configs, to_file_type, to_file_type_from_filename, ConfigFileType, ConfigFormat};
use crate::controller::utils::parsers::text_to_json::try_parse_file_to_json;
use crate::controller::v1alpha1::configuration_discoverer::ConfigurationDiscoverer;

#[derive( Debug, Clone, Deserialize, Serialize, JsonSchema)]
enum SupportedClaimResourceType {
    ConfigMap,
    Secret,
}

#[derive( Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub enum ConfigInjectionStrategy {
    Merge,
    Fallback,
}
#[derive( Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub enum SupportedConfigurationStoreResourceType {
    ConfigurationStore,
    ClusterConfigurationStore,
}
#[derive( Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub enum ClaimCreationPolicy {
    Owned,
    Orphan,
    Merge,
    None
}
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ClaimConfigurationStoreRef {
    pub name: String,
    pub kind: SupportedConfigurationStoreResourceType
}


#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ClaimTargetRef {
    pub name: String,
    pub creationPolicy: ClaimCreationPolicy
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ClaimRefParametrization {
    pub configurationStoreRef: ClaimConfigurationStoreRef,
    pub configurationStoreParams : Option<HashMap<String, String>>
}
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ClaimRef {
    //TODO: Json schema to validate against
    // pub schema: Option<String> either schema or url to schema (schema refs? )
    pub from: Vec<ClaimRefParametrization>,
    pub strategy: Option<ConfigInjectionStrategy>
}

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(group = "external-config.com", version="v1alpha1", kind = "ConfigMapClaim", namespaced, shortname = "cmc" )]
#[kube(status = "ConfigurationSourceStatus")]
pub struct ConfigMapClaimSpec {
    pub data: HashMap<String, ClaimRef>,
    pub target: ClaimTargetRef,
    pub refreshInterval: Option<RefreshInterval>,

}
pub trait HasTarget {
    fn get_target(&self) -> &ClaimTargetRef;
}
pub trait Refreshable {
    fn get_refresh_interval(&self) -> Duration;
}


impl Refreshable for ConfigMapClaim {
    fn get_refresh_interval(&self) -> Duration {
        match &self.spec.refreshInterval {
            Some(value ) => Duration::from_secs(value.as_seconds()),
            None => {Duration::from_secs(5*60)}
        }
    }
}

impl HasTarget for ConfigMapClaim {
    fn get_target(&self) -> &ClaimTargetRef {
        &self.spec.target
    }
}


#[async_trait]
impl ConfigurationDiscoverer<ConfigMap> for ConfigMapClaim {
    async fn create_resource_spec(&self, ctx: Arc<Data>) -> std::result::Result<ConfigMap, Error> {
        let name = self.spec.target.name.clone();
        let namespace = <Self as kube::ResourceExt>::namespace(self).unwrap();
        let mut data: BTreeMap<String, String> = BTreeMap::new();

        for (file, refs) in &self.spec.data {
            Self::compose_file(self, ctx.clone(), &refs, &namespace, file, &mut data).await?
        }

        self.record_event(ctx.client.clone(), "Reconcile", "Successful", EventType::Normal).await;

        Ok(ConfigMap {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(name),
                namespace: Some(namespace),
                owner_references: Some(vec![self.controller_owner_ref(&()).unwrap()]),
                // finalizers: Some(vec![String::from("test.io/documents.kube.rs")]),
                ..Default::default()
            },
            data: Some(data),
            ..Default::default()
        })
    }
}

impl Default for ConfigMapClaim {
    fn default() -> Self {
        ConfigMapClaim {
            // Provide default values for the necessary fields.
            // Adjust the fields as per your struct definition.
            status: Some(ConfigurationSourceStatus::default()),
            metadata: Default::default(),
            spec: Default::default(),
        }
    }
}
impl Default for ConfigMapClaimSpec {
    fn default() -> Self {
        ConfigMapClaimSpec {
            data: HashMap::new(),
            target: ClaimTargetRef::default(),
            refreshInterval: None,
        }
    }
}

impl Default for ClaimTargetRef {
    fn default() -> Self {
        ClaimTargetRef {
            name: String::new(),
            creationPolicy: ClaimCreationPolicy::Merge
        }
    }
}


#[async_trait]
impl IReconcilable for ConfigMapClaim {

    async fn reconcile(&self,  ctx: Arc<Data>) -> Result<Action> {

        let client = ctx.client.clone();
        match ConfigurationDiscoverer::<ConfigMap>::reconcile(self, ctx).await {
            Ok(action) => Ok(action),
            Err(e) => {
                self.record_event(client, "Reconcile", &e.to_string(), EventType::Warning).await?;
                Err(e)
            }
        }
    }
    async fn cleanup(&mut self,  ctx: Arc<Data>) -> Result<Action> {

        ConfigurationDiscoverer::<ConfigMap>::cleanup(self, ctx).await

    }

}

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(group = "external-config.com", version="v1alpha1", kind = "SecretClaim", namespaced, shortname = "sc" )]
#[kube(status = "ConfigurationSourceStatus")]
pub struct SecretClaimSpec {
    pub data: HashMap<String, ClaimRef>,
    pub target: ClaimTargetRef,
    pub refreshInterval: Option<RefreshInterval>,
}

impl Refreshable for SecretClaim {
    fn get_refresh_interval(&self) -> Duration {
        match &self.spec.refreshInterval {
            Some(value ) => Duration::from_secs(value.as_seconds()),
            None => {Duration::from_secs(5*60)}
        }
    }
}

impl HasTarget for SecretClaim {
    fn get_target(&self) -> &ClaimTargetRef {
        &self.spec.target
    }
}


#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct ConfigurationSourceStatus {
    pub last_synced: Option<String>,
}

impl Default for SecretClaimSpec {
    fn default() -> Self {
        SecretClaimSpec {
            data: HashMap::new(),
            target: ClaimTargetRef::default(),
            refreshInterval: None,
        }
    }
}

impl Default for ConfigurationSourceStatus {
    fn default() -> Self {
        Self {
            last_synced: None,
        }
    }
}

#[async_trait]
impl ConfigurationDiscoverer<Secret> for SecretClaim {
    async fn create_resource_spec(&self, ctx: Arc<Data>) -> std::result::Result<Secret, Error> {
        let name = self.spec.target.name.clone();
        let namespace = <Self as kube::ResourceExt>::namespace(self).unwrap();
        let mut data: BTreeMap<String, String> = BTreeMap::new();


        for (file, refs) in &self.spec.data {
            Self::compose_file(self, ctx.clone(), &refs, &namespace, file, &mut data).await?
        }


        let encoded_data: BTreeMap<String, ByteString> = data
            .iter()
            .map(|(k,v)| {
                let enc = STANDARD.encode(v).into_bytes();
                (k.clone(), ByteString(enc.to_vec()))
            })
            .collect();


        self.record_event(ctx.client.clone(), "Reconcile", "Successful", EventType::Normal).await;

        Ok(Secret {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(name),
                namespace: Some(namespace),
                owner_references: Some(vec![self.controller_owner_ref(&()).unwrap()]),
                // finalizers: Some(vec![String::from("test.io/documents.kube.rs")]),
                ..Default::default()
            },
            data: Some(encoded_data),
            ..Default::default()
        })
    }
}


impl Default for SecretClaim {
    fn default() -> Self {
        SecretClaim {
            // Provide default values for the necessary fields.
            // Adjust the fields as per your struct definition.
            status: Some(ConfigurationSourceStatus::default()),
            metadata: Default::default(),
            spec: Default::default(),
        }
    }
}

#[async_trait]
impl IReconcilable for SecretClaim {


    async fn reconcile(&self,  ctx: Arc<Data>) -> Result<Action> {

        let client = ctx.client.clone();
        match ConfigurationDiscoverer::<Secret>::reconcile(self, ctx).await {
            Ok(action) => Ok(action),
            Err(e) => {
                self.record_event(client, "Reconcile", &e.to_string(), EventType::Warning).await?;
                Err(e)
            }
        }
    }
    async fn cleanup(&mut self,  ctx: Arc<Data>) -> Result<Action> {

        ConfigurationDiscoverer::<Secret>::cleanup(self, ctx).await

    }

}
