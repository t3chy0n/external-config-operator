
use std::env;

pub struct Config {}

impl Config {
    pub fn kubernetes_namespace() -> Option<String>{
        env::var("KUBERNETES_NAMESPACE").map(|v| Some(v)).unwrap_or(None)
    }
    pub fn kubernetes_pod_name() -> Option<String>{
        env::var("KUBERNETES_POD_NAME").map(|v| Some(v)).unwrap_or(None)
    }
    pub fn is_in_pod() -> bool {
        env::var("KUBERNETES_POD_NAME")
            .map(|x| true)
            .unwrap_or_else(|_| false)
    }
    pub fn enable_leader_election() -> bool{
        env::var("ENABLE_LEADER_ELECTION")
            .map(|x| matches!(x.to_ascii_lowercase().as_str(), "1" | "true"))
            .unwrap_or(false)
    }
    pub fn opentelemetry_endpoint_url() -> Option<String>{
        env::var("OPENTELEMETRY_ENDPOINT_URL").map(|v| Some(v)).unwrap_or(None)
    }
    pub fn new() -> Self {
        Config {}
    }
}