use std::fmt::Debug;
use std::sync::Arc;
use async_trait::async_trait;
use k8s_openapi::NamespaceResourceScope;
use kube::{Client, Resource, ResourceExt};
use kube::runtime::controller::Action;
use kube::runtime::events::{Event, EventType, Recorder, Reporter};
use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::controller::utils::context::Context;
use crate::controller::utils::crd::HasData;
use super::lib::{Error, Result};

#[async_trait]
pub trait IReconcilable: Resource<DynamicType = ()> {
    async fn reconcile(&self, ctx: Arc<Context>) -> Result<Action>;
    async fn cleanup(&mut self, ctx: Arc<Context>) -> Result<Action>;

    async fn record_event(
        &self,
        client: Arc<Client>,
        reason: &str,
        message: &str,
        event_type: EventType,  // Can be `Normal` or `Warning`
    ) -> Result<(), Error> {
        // Create an event recorder
        let reporter = Reporter {
            controller: "external-configuration".into(),
            instance: Some("Test".into())
        };

        let resource_ref = self.object_ref(&());
        let recorder = Recorder::new((*client).clone(), reporter, resource_ref);

        // Create and publish an event
        let event = Event {
            type_: event_type,
            reason: reason.to_string(),
            note: Some(message.to_string()),
            action: "Test".to_string(),
            secondary: None
        };

        let res = recorder.publish(event).await;

        println!("Event recorded: {}", message);
        Ok(())
    }
}

pub trait ReconcilableTargetTypeBounds:
    Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + Clone
        + std::fmt::Debug
        + Default
        + HasData
        + Sync
        + Send
        + serde::de::DeserializeOwned
        + serde::Serialize
    {}


pub trait ControllerReconcilableTargetTypeBounds:
    Resource<Scope = NamespaceResourceScope, DynamicType = ()>
    + ResourceExt
    + IReconcilable
    + Send
    + Sync
    + Clone
    + Serialize
    + Debug
    + DeserializeOwned
    + 'static

{}

// Automatically implement the helper trait for any type that satisfies the constraints
impl<T> ReconcilableTargetTypeBounds for T where
    T: Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + Clone
        + std::fmt::Debug
        + Default
        + HasData
        + Sync
        + Send
        + serde::de::DeserializeOwned
        + serde::Serialize
{}
impl<T> ControllerReconcilableTargetTypeBounds for T where
    T:     Resource<Scope = NamespaceResourceScope, DynamicType = ()>
        + ResourceExt
        + IReconcilable
        + Send
        + Sync
        + Clone
        + Serialize
        + Debug
        + DeserializeOwned
        + 'static
{}