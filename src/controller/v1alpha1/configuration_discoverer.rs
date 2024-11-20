use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::Arc;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use kube::{Api, Client, Resource, ResourceExt};

use k8s_openapi::{ByteString, NamespaceResourceScope};
use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use kube::api::{Patch, PatchParams, PostParams};
use kube::runtime::controller::Action;
use kube::runtime::events::EventType;
use serde_json::json;
use tracing::log::info;
use crate::contract::clients::ICrdClient;
use super::crd::claim::{ClaimRef, ClaimRefParametrization, ConfigInjectionStrategy, SupportedConfigurationStoreResourceType};
use crate::controller::utils::parsers::text_to_json::try_parse_file_to_json;
use crate::controller::utils::file_format::{convert_to_format, convert_to_json, merge_configs, to_file_type, to_file_type_from_filename, ConfigFileType, ConfigFormat};

use crate::contract::lib::{Error, Result};
use crate::contract::ireconcilable::{IReconcilable, ReconcilableTargetTypeBounds};
use crate::controller::controller::DOCUMENT_FINALIZER;
use crate::controller::utils::context::Data;
use crate::controller::utils::crd::HasData;
use crate::controller::v1alpha1::crd::claim::{HasTarget, Refreshable};
use crate::controller::v1alpha1::crd::configuration_store::{ClusterConfigurationStore, ConfigurationStore, Provider};

#[async_trait]
pub trait ConfigurationDiscoverer<TargetType>: IReconcilable + Sized + HasTarget + Refreshable where
    TargetType: ReconcilableTargetTypeBounds
{
    async fn compose_file(
        &self,
        ctx: Arc<Data>,
        claim_ref: &ClaimRef,
        namespace: &str,
        file: &str,
        data: &mut BTreeMap<String, String>
    ) -> Result<(), Error> {
        match &claim_ref.strategy.as_ref().unwrap_or(&ConfigInjectionStrategy::Fallback) {
            ConfigInjectionStrategy::Merge => self.apply_merge_strategy(ctx, claim_ref, namespace, file, data).await,
            ConfigInjectionStrategy::Fallback => self.apply_fallback_strategy(ctx, claim_ref, namespace, file, data).await,
        }
    }
    async fn apply_merge_strategy(
        &self,
        ctx: Arc<Data>,
        claim_ref: &ClaimRef,
        namespace: &str,
        file: &str,
        data: &mut BTreeMap<String, String>,
    ) -> Result<(), Error> {
        let mut merged_config: Option<ConfigFormat> = None;
        let mut first_file_format: Option<ConfigFileType> = to_file_type_from_filename(file);

        for store_ref in &claim_ref.from {
            let result = self.process_store_ref(ctx.clone(), store_ref, namespace, file).await?;

            if first_file_format.is_none() {
                first_file_format = Some(to_file_type(&result)?);
            }

            merged_config = Some(if let Some(existing) = merged_config {
                merge_configs(existing, result)?
            } else {
                result
            });
        }

        if let Some(merged) = merged_config {
            if let Some(file_format) = first_file_format {
                let final_result = convert_to_format(&merged, &file_format)?;
                data.insert(file.to_string(), final_result);
            } else {
                return Err(Error::UnsupportedFileType());
            }
        }
        Ok(())
    }

    async fn apply_fallback_strategy(
        &self,
        ctx: Arc<Data>,
        claim_ref: &ClaimRef,
        namespace: &str,
        file: &str,
        data: &mut BTreeMap<String, String>,
    ) -> Result<(), Error> {
        for store_ref in &claim_ref.from {
            if let Ok(file_data) = self.process_store_ref(ctx.clone(), store_ref, namespace, file).await {
                data.insert(file.to_string(), convert_to_format(&file_data, &ConfigFileType::Json)?);
                return Ok(());
            }
        }
        Err(Error::ConfigStoreError())
    }
    async fn process_store_ref(
        &self,
        ctx: Arc<Data>,
        store_ref: &ClaimRefParametrization,
        namespace: &str,
        file: &str
    ) -> Result<ConfigFormat, Error> {

        let provider = match store_ref.configurationStoreRef.kind {
            SupportedConfigurationStoreResourceType::ClusterConfigurationStore => {
                let store = ctx.v1alpha1.get_cluster_config_store(&store_ref.configurationStoreRef.name).await?;
                store.spec.provider
            }
            SupportedConfigurationStoreResourceType::ConfigurationStore => {
                let store = ctx.v1alpha1.get_config_store(&store_ref.configurationStoreRef.name,namespace).await?;
                store.spec.provider
            }
        };

        let config_store = provider.get_config_store();
        let file = config_store.get_config(
            store_ref.configurationStoreParams.clone(),
            None
        ).await?;

        let parsed_config = try_parse_file_to_json(&file)?;
        convert_to_json(&parsed_config)
    }

    async fn reconcile(&self, ctx: Arc<Data>) -> Result<Action> {
        let client = ctx.client.clone();
        let namespace = <Self as ResourceExt>::namespace(self).unwrap();
        let name = self.name_any();

        let resources: Api<TargetType> = Api::namespaced((*client).clone(), &namespace);
        info!("Reconciling resource: {} in namespace: {}", name, namespace);
        let target = self.get_target();

        match resources.get(&target.name).await {
            Ok(existing_resource) => {
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

    async fn cleanup(&mut self, ctx: Arc<Data>) -> Result<Action> {
        let client = ctx.client.clone();
        let namespace = <Self as ResourceExt>::namespace(self).unwrap();
        let target = self.get_target();
        let name = &target.name;

        let resources: Api<TargetType> = Api::namespaced((*client).clone(), &namespace);
        if let Ok(mut resource) = resources.get(name).await {
            if let Some(ref mut finalizers) = resource.meta_mut().finalizers {
                finalizers.retain(|f| f != DOCUMENT_FINALIZER);
                resources.replace(name, &PostParams::default(), &resource).await.map_err(Error::KubeError)?;
            }
        }

        Ok(Action::await_change())
    }

    async fn create_resource_spec(&self, ctx: Arc<Data>) -> Result<TargetType, Error>;
}

