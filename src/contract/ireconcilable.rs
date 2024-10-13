use std::sync::Arc;
use async_trait::async_trait;
use kube::{Client, Resource};
use kube::runtime::controller::Action;
use kube::runtime::events::{Event, EventType, Recorder, Reporter};
use crate::controller::utils::context::Data;
use super::lib::{Error, Result};

#[async_trait]
pub trait IReconcilable: Resource<DynamicType = ()> {
    async fn reconcile(&self,  ctx: Arc<Data>) -> Result<Action>;
    async fn cleanup(&mut self,  ctx: Arc<Data>) -> Result<Action>;

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