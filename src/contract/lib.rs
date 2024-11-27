use java_properties::PropertiesError;
use kube::core::ErrorResponse;
use thiserror::Error;
use tokio::task::JoinError;

pub type Result<T, E = Error > = std::result::Result<T, E >;
#[derive(Error, Debug)]
pub enum Error {
    #[error("SerializationError: {0}")]
    JsonSerializationError(#[source] serde_json::Error),

    #[error("SerializationError: {0}")]
    TomlSerializationError(#[source] toml::ser::Error),

    #[error("SerializationError: {0}")]
    YamlSerializationError(#[source] serde_yaml::Error),
    #[error("SerializationError: {0}")]

    PropertiesSerializationError(#[source] PropertiesError),

    #[error("SerializationError: {0}")]
    EnvFileSerializationError(#[source] std::io::Error),


    #[error("Http Server Error: {0}")]
    HttpServerError(#[source] std::io::Error),

    #[error("Config Store Error: ")]
    ConfigStoreError(/* #[source] dyn std::error::Error)*/),

    #[error("Http Store Error: {0} ")]
    HttpConfigStoreError( #[source] reqwest::Error),

    #[error("Http Store Client Error: {0} ")]
    HttpConfigStoreClientError(#[source] std::io::Error),

    #[error("Http Store Server Error: {0} ")]
    HttpConfigStoreServerError(#[source] std::io::Error),

    #[error("Kube Error: {0}")]
    KubeError(#[source] kube::Error),

    #[error("Kube Client Error: {0}")]
    KubeClientError(#[source] ErrorResponse),

    #[error("Unsupported configuration file format")]
    UnsupportedFileType(),

    #[error("Another pod holds the lease")]
    LeaseHeldByAnotherPod(),
    
    #[error("Operation was cancelled")]
    Cancelled,

    #[error("Incompatible file formats")]
    IncompatibleFileTypes(),

    #[error("Error when parsing config file")]
    ParseError(),

    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    FinalizerError(#[source] Box<kube::runtime::finalizer::Error<Error>>),

    #[error("IllegalDocument")]
    IllegalDocument,

    #[error("Thread join error")]
    ThreadJoinError(#[source] JoinError),

    #[error("Traceing Error")]
    TracingError(),


}


impl Error {
    pub fn metric_label(&self) -> String {
        format!("{self:?}").to_lowercase()
    }
}