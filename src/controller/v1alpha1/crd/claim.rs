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
use crate::contract::ireconcilable::IReconcilable;
use crate::controller::utils::crd::{HasData, RefreshInterval};
use crate::controller::utils::context::Data;
use crate::controller::v1alpha1::crd::configuration_store::{ClusterConfigurationStore, ConfigStoreFetcherAdapter, ConfigurationStore, Provider};
use crate::contract::lib::{Error, Result};
use crate::controller::controller::DOCUMENT_FINALIZER;
use base64::engine::general_purpose::STANDARD;
use crate::controller::utils::file_format::{convert_to_format, convert_to_json, merge_configs, to_file_type, to_file_type_from_filename, ConfigFileType, ConfigFormat};
use crate::controller::utils::parsers::text_to_json::try_parse_file_to_json;

#[derive( Debug, Clone, Deserialize, Serialize, JsonSchema)]
enum SupportedClaimResourceType {
    ConfigMap,
    Secret,
}

#[derive( Debug, Clone, Deserialize, Serialize, JsonSchema)]
enum ConfigInjectionStrategy {
    Merge,
    Fallback,
}
#[derive( Debug, Clone, Deserialize, Serialize, JsonSchema)]
enum SupportedConfigurationStoreResourceType {
    ConfigurationStore,
    ClusterConfigurationStore,
}
#[derive( Debug, Clone, Deserialize, Serialize, JsonSchema)]
enum ClaimCreationPolicy {
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
impl Default for ConfigurationSourceStatus {
    fn default() -> Self {
        Self {
            last_synced: None,
        }
    }
}

#[async_trait]
pub trait ConfigurationDiscoverer<TargetType>: IReconcilable + Sized + HasTarget + Refreshable where
    TargetType: Resource<DynamicType = (), Scope = NamespaceResourceScope>
    + Clone
    + std::fmt::Debug
    + Default
    + HasData
    + Sync
    + Send
    + serde::de::DeserializeOwned
    + serde::Serialize,

{

    async fn compose_file(
        &self,
        client: Arc<Client>,
        claim_ref: &ClaimRef,
        namespace: &str,
        file: &str,
        data: &mut BTreeMap<String, String>
    ) -> Result<(), Error> {
        match &claim_ref.strategy {
            Some(strategy) => match strategy {
                ConfigInjectionStrategy::Merge => {
                    self.apply_merge_strategy(client.clone(), claim_ref, namespace, file, data).await
                }
                ConfigInjectionStrategy::Fallback => {
                    self.apply_fallback_strategy(client.clone(), claim_ref, namespace, file, data).await
                }
            },
            None => Ok(()),
        }
    }


    async fn apply_merge_strategy(
        &self,
        client: Arc<Client>,
        claim_ref: &ClaimRef,
        namespace: &str,
        file: &str,
        data: &mut BTreeMap<String, String>,
    ) -> Result<(), Error> {
        let mut merged_config: Option<ConfigFormat> = None;
        let mut first_file_format: Option<ConfigFileType> = to_file_type_from_filename(file);

        for store_ref in &claim_ref.from {
            // Fetch the configuration as a string
            let result = Self::process_store_ref(self, client.clone(), store_ref, namespace, file).await?;

            // Try to parse the fetched result into a ConfigFormat (JSON, TOML, YAML, etc.)
            let parsed_config = try_parse_file_to_json(&result)?;

            let json_config = convert_to_json(&parsed_config)?;

            if first_file_format.is_none() {
                first_file_format = Some(to_file_type(&parsed_config)?);
            }
            // If we haven't initialized merged_config, set it to the first fetched configuration
            if merged_config.is_none() {
                merged_config = Some(json_config);
            } else {
                // If we already have a merged configuration, merge the new config into it
                merged_config = Some(merge_configs(merged_config.take().unwrap(), json_config)?);
            }

        }

        if let Some(merged) = merged_config {
            if let Some(file_format) = first_file_format {
                // Convert the final merged configuration back to the original format
                let final_result = convert_to_format(&merged, &file_format)?;

                // Insert the merged result into the data map
                data.insert(file.to_string(), final_result);
            } else {
                // Handle the error when no file format was determined
                return Err(Error::UnsupportedFileType());
            }
        }
        Ok(())
    }


    async fn apply_fallback_strategy(
        &self,
        client: Arc<Client>,
        claim_ref: &ClaimRef,
        namespace: &str,
        file: &str,
        data: &mut BTreeMap<String, String>,
    ) -> Result<(), Error> {
        let mut success = false;
        for store_ref in &claim_ref.from {
            let result = Self::process_store_ref(self, client.clone(), &store_ref, namespace, file).await;
            match result {
                Ok(file_data) => {
                    data.insert(file.to_string(), file_data);
                    success = true;
                    break;
                }
                Err(_) => continue,
            }
        }
        if success {
            Ok(())
        } else {
            Err(Error::ConfigStoreError()) // TODO: Provide a more descriptive error message.
        }
    }
    /// Process the store reference and fetch the configuration data.
    async fn process_store_ref(
        &self,
        client: Arc<Client>,
        store_ref: &ClaimRefParametrization,
        namespace: &str,
        file: &str
    ) -> Result<String, Error> {
        match store_ref.configurationStoreRef.kind {
            SupportedConfigurationStoreResourceType::ClusterConfigurationStore => {
                self.fetch_cluster_configuration_store(client.clone(), store_ref, file).await
            }
            SupportedConfigurationStoreResourceType::ConfigurationStore => {
                self.fetch_configuration_store(client.clone(), store_ref, namespace, file).await
            }
        }
    }

    /// Fetch data from ClusterConfigurationStore and update the data map.
    async fn fetch_cluster_configuration_store(
        &self,
        client: Arc<Client>,
        store_ref: &ClaimRefParametrization,
        file: &str,
    ) -> Result<String, Error> {
        let store = Api::<ClusterConfigurationStore>::all((*client).clone())
            .get(&store_ref.configurationStoreRef.name).await
            .map_err(|e|  { Error::KubeError(e) })?;

        let config_store = store.spec.provider.get_config_store();
        config_store.get_config("asd".to_string()).await
    }

    /// Fetch data from ConfigurationStore and update the data map.
    async fn fetch_configuration_store(
        &self,
        client: Arc<Client>,
        store_ref: &ClaimRefParametrization,
        namespace: &str,
        file: &str,
    ) -> Result<String, Error> {
        let store=  Api::<ConfigurationStore>::namespaced((*client).clone(), namespace)
            .get(&store_ref.configurationStoreRef.name).await
            .map_err(|e|  { Error::KubeError(e) })?;


        let config_store = store.spec.provider.get_config_store();
        config_store.get_config("asd".to_string()).await
    }

    async fn reconcile(&self,  ctx: Arc<Data>) -> Result<Action> {

        let client = ctx.client.clone();
        let ns = <Self as kube::ResourceExt>::namespace(self).unwrap();
        let name = self.name_any();

        let resources: Api<TargetType> = Api::<TargetType>::namespaced((*client).clone(), &ns);

        info!("Reconciling ConfigMapClaim: {} in namespace: {}", name, ns);
        // Check if the ConfigMap exists
        let target = self.get_target();

        match resources.get(&target.name).await {
            Ok(existing_config_map) => {
                // ConfigMap exists, check if it needs updating
                let desired_config_map = Self::create_resource_spec(self, client.clone()).await?; // Define this method to create your desired ConfigMap spec

                if existing_config_map.get_data() != desired_config_map.get_data() {
                    // Update the ConfigMap if data differs
                    let patch = Patch::Apply(json!(&desired_config_map));
                    let params = PatchParams::apply("configmap-claim-controller").force();
                    match resources.patch(&target.name, &params, &patch).await {
                        Ok(..) => Ok(()),
                        Err(kube::Error::Api(e)) if e.code == 404 => {
                            Err(Error::KubeClientError(e))
                        }
                        Err(e) => {
                            // Other errors
                            return Err(Error::KubeError(e));
                        }
                    }?
                }
            }
            Err(kube::Error::Api(ref e)) if e.code == 404 => {
                // ConfigMap doesn't exist, create it
                let config_map = self.create_resource_spec(client.clone()).await?; // Define this method to create your desired ConfigMap spec
                match resources.create(&PostParams::default(), &config_map).await {
                    Ok(..) => Ok(()),
                    Err(kube::Error::Api(e)) if e.code == 404 => {
                        Err(Error::KubeClientError(e))
                    }
                    Err(e) => {
                        // Other errors
                        return Err(Error::KubeError(e));
                    }
                }?
            }
            Err(e) => {
                // Other errors
                return Err(Error::KubeError(e));
            }
        }

        Ok(Action::requeue(self.get_refresh_interval()))
    }

    async fn cleanup(&mut self,  ctx: Arc<Data>) -> Result<Action> {
        let client = ctx.client.clone();
        let ns = <Self as kube::ResourceExt>::namespace(self).unwrap();
        let target = self.get_target();
        let name = &target.name;

        // Get a handle to the ConfigMap API
        let resources: Api<TargetType> = Api::namespaced((*client).clone(), &ns);

        let mut resource = resources.get(&name).await.unwrap();

        let metadata = resource.get_metadata_mut();

        if let Some(ref mut finalizers) =  &mut metadata.finalizers {
            finalizers.retain(|f| f != DOCUMENT_FINALIZER);

            match resources.replace(&name, &PostParams::default(), &resource).await {
                Ok(existing_config_map) => {

                }
                Err(kube::Error::Api(e)) if e.code == 404 => {

                    // Other errors
                    return Err(Error::KubeClientError(e));
                }
                Err(e) => {
                    // Other errors
                    return Err(Error::KubeError(e));
                }
            }
        }
        // // Check the deletion policy in the claim
        // if let ClaimCreationPolicy::Owned = self.spec.target.creationPolicy  {
        //     // Delete the ConfigMap
        //
        //     match config_maps.delete(name, &DeleteParams::default()).await {
        //         Ok(_) => Ok(Action::await_change()),
        //         Err(kube::Error::Api(ref e)) if e.code == 404 => Ok(Action::await_change()), // ConfigMap already deleted
        //         Err(e) => Err(Error::KubeError(e)),
        //     }?;
        // } else {
        //     return Ok(Action::await_change())
        // }
        Ok(Action::await_change())
    }
    async fn create_resource_spec(&self, client: Arc<Client>) -> Result<TargetType, Error>;
}

#[async_trait]
impl ConfigurationDiscoverer<ConfigMap> for ConfigMapClaim {
    async fn create_resource_spec(&self, client: Arc<Client>) -> Result<ConfigMap, Error> {
        let name = self.spec.target.name.clone();
        let namespace = <Self as kube::ResourceExt>::namespace(self).unwrap();
        let mut data: BTreeMap<String, String> = BTreeMap::new();

        for (file, refs) in &self.spec.data {
            Self::compose_file(self, client.clone(), &refs, &namespace, file, &mut data).await?
        }

        self.record_event(client.clone(), "Reconcile", "Successful", EventType::Normal).await;

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

#[async_trait]
impl ConfigurationDiscoverer<Secret> for SecretClaim {
    async fn create_resource_spec(&self, client: Arc<Client>) -> Result<Secret, Error> {
        let name = self.spec.target.name.clone();
        let namespace = <Self as kube::ResourceExt>::namespace(self).unwrap();
        let mut data: BTreeMap<String, String> = BTreeMap::new();


        for (file, refs) in &self.spec.data {
            Self::compose_file(self, client.clone(), &refs, &namespace, file, &mut data).await?
        }


        let encoded_data: BTreeMap<String, ByteString> = data
            .iter()
            .map(|(k,v)| {
                let enc = STANDARD.encode(v).into_bytes();
                (k.clone(), ByteString(enc.to_vec()))
            })
            .collect();


        self.record_event(client.clone(), "Reconcile", "Successful", EventType::Normal).await;

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
#[async_trait]
impl IReconcilable for SecretClaim {


    async fn reconcile(&self,  ctx: Arc<Data>) -> Result<Action> {

        let client = ctx.client.clone();
        match ConfigurationDiscoverer::<Secret>::reconcile(self, ctx).await {
            Ok(action) => Ok(action),
            Err(e) => { Err(e)
            }
        }
    }
    async fn cleanup(&mut self,  ctx: Arc<Data>) -> Result<Action> {

        ConfigurationDiscoverer::<Secret>::cleanup(self, ctx).await

    }

}
