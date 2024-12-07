use chrono::Duration;
use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use schemars::schema::{InstanceType, Metadata, Schema, SchemaObject};
use schemars::JsonSchema;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::str::FromStr;
#[derive(Debug, Clone, PartialEq)]
pub struct RefreshInterval(Duration);

impl RefreshInterval {
    /// Returns the wrapped `Duration` object.
    pub fn as_duration(&self) -> Duration {
        self.0
    }

    /// Returns the duration as the number of seconds.
    pub fn as_seconds(&self) -> u64 {
        if self.0.num_seconds() < 0 {
            0 // or handle this case as per your logic
        } else {
            self.0.num_seconds() as u64
        }
    }

    /// Returns the duration as the number of milliseconds.
    pub fn as_millis(&self) -> u128 {
        if self.0.num_seconds() < 0 {
            0 // or handle this case as per your logic
        } else {
            self.0.num_milliseconds() as u128
        }
    }
}
impl FromStr for RefreshInterval {
    type Err = humantime::DurationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match humantime::parse_duration(s) {
            Ok(d) => Ok(RefreshInterval(Duration::from_std(d).unwrap())),
            Err(e) => Err(e), // Directly return the humantime error
        }
    }
}

impl JsonSchema for RefreshInterval {
    fn schema_name() -> String {
        "RefreshInterval".to_string()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()), // The type will be a string
            metadata: Some(Box::new(Metadata {
                description: Some("A time duration like '1h', '15m', '2600s'".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

impl Serialize for RefreshInterval {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration_str = humantime::format_duration(self.0.to_std().unwrap()).to_string();
        serializer.serialize_str(&duration_str)
    }
}

impl<'de> Deserialize<'de> for RefreshInterval {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

pub trait HasData {
    fn get_data(&self) -> Option<BTreeMap<String, String>>;
    fn get_metadata_mut(&mut self) -> &mut ObjectMeta;
}

impl HasData for ConfigMap {
    fn get_data(&self) -> Option<BTreeMap<String, String>> {
        self.data.clone()
    }
    fn get_metadata_mut(&mut self) -> &mut ObjectMeta {
        &mut self.metadata
    }
}

impl HasData for Secret {
    fn get_data(&self) -> Option<BTreeMap<String, String>> {
        self.data.as_ref().map(|data| {
            data.iter()
                .map(|(k, v)| {
                    // Convert the ByteString into &[u8] and then to a String
                    (k.clone(), String::from_utf8_lossy(&v.0).to_string())
                })
                .collect()
        })
    }

    fn get_metadata_mut(&mut self) -> &mut ObjectMeta {
        &mut self.metadata
    }
}
