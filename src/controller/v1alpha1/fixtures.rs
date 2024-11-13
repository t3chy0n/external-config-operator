#[cfg(test)]
pub mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use crate::controller::controller::apply_from_yaml;
    use kube::Client;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path, query_param};

    #[derive(Clone)]
    pub struct MockConfig {
        pub status_code: u16,
        pub response_body: String,
        pub query_params: Option<HashMap<String, String>>, // Optional query parameters for conditional responses
    }

    impl MockConfig {
        pub fn new(status_code: u16, response_body: &str, query_params: Option<HashMap<String, String>>) -> Self {
            MockConfig {
                status_code,
                response_body: response_body.to_string(),
                query_params,
            }
        }

        pub fn default() -> Self {
            MockConfig {
                status_code: 200,
                response_body: String::from("{}"),
                query_params: None
            }
        }
        pub fn query_param_response(param: &str, value: &str, response: &str) -> Self {
            let mut query_params = HashMap::new();
            query_params.insert(String::from(param),String::from(value));
            MockConfig {
                status_code: 200,
                response_body: String::from(response),
                query_params: Some(query_params)
            }
        }
        pub fn success_with_body(body: &str) -> Self {
            MockConfig {
                status_code: 200,
                response_body: String::from(body),
                query_params: None
            }
        }
        pub fn not_found_with_body(body: &str) -> Self {
            MockConfig {
                status_code: 404,
                response_body: String::from(body),
                query_params: None
            }
        }
        pub fn not_found() -> Self {
            MockConfig {
                status_code: 404,
                response_body: String::from(""),
                query_params: None
            }
        }
        pub fn internal_server_error_with_body(body: &str) -> Self {
            MockConfig {
                status_code: 500,
                response_body: String::from(body),
                query_params: None
            }
        }
        pub fn internal_server_error() -> Self {
            MockConfig {
                status_code: 500,
                response_body: String::from("body"),
                query_params: None
            }
        }
    }
    pub struct ResourceFixture {
        name: String,
        namespace: Option<String>,
        kind: String,
        provider_url: Option<String>,
        data: Option<HashMap<String, String>>,
    }

    pub struct ConfigurationStoreRef {
        pub kind: String,
        pub name: String,
        pub params: HashMap<String, String>,
    }

    pub struct MockServerManager {
        mock_servers: HashMap<String, MockServer>,
    }

    impl MockServerManager {
        pub async fn new() -> Self {
            MockServerManager {
                mock_servers: HashMap::new(),
            }
        }

        pub async fn create_mock_server(&mut self, key: &str, configs: Vec<MockConfig>) -> String {
            let mock_server = MockServer::start().await;

            for config in configs {
                let mut mock = Mock::given(method("GET")).and(path("/config"));

                // Add query parameter matchers if specified in the config
                if let Some(params) = &config.query_params {
                    for (param, value) in params {
                        mock = mock.and(query_param(param, value));
                    }
                }

                // Set the response for this specific configuration
                mock.respond_with(ResponseTemplate::new(config.status_code).set_body_string(config.response_body))
                    .mount(&mock_server)
                    .await;
            }

            let uri = mock_server.uri();
            self.mock_servers.insert(key.to_string(), mock_server);
            uri
        }
    }

    pub struct ControllerFixtures {
        resources: Vec<ResourceFixture>,
        client: Arc<Client>,
        mock_manager: MockServerManager,
    }

    impl ControllerFixtures {
        pub async fn new(client: Arc<Client>) -> Self {
            ControllerFixtures {
                resources: vec![],
                client,
                mock_manager: MockServerManager::new().await,
            }
        }

        pub async fn add_configuration_store(&mut self, name: &str, namespace: &str, configs: Vec<MockConfig>) -> &Self {
            let mock_url = self.mock_manager.create_mock_server(name, configs).await;
            self.resources.push(ResourceFixture {
                name: name.to_string(),
                namespace: Some(namespace.to_string()),
                kind: "ConfigurationStore".to_string(),
                provider_url: Some(mock_url + "/config"),
                data: None,
            });
            self
        }

        pub async fn add_cluster_configuration_store(&mut self, name: &str, configs: Vec<MockConfig>) -> &Self {
            let mock_url = self.mock_manager.create_mock_server(name, configs).await;
            self.resources.push(ResourceFixture {
                name: name.to_string(),
                namespace: None,
                kind: "ClusterConfigurationStore".to_string(),
                provider_url: Some(mock_url + "/config"),
                data: None,
            });
            self
        }

        pub fn add_config_map_claim(
            &mut self,
            name: &str,
            namespace: &str,
            data: HashMap<String, String>,
        ) -> &Self {
            self.resources.push(ResourceFixture {
                name: name.to_string(),
                namespace: Some(namespace.to_string()),
                kind: "ConfigMapClaim".to_string(),
                provider_url: None,
                data: Some(data),
            });
            self
        }
        pub fn add_secret_claim(
            &mut self,
            name: &str,
            namespace: &str,
            data: HashMap<String, String>
        ) -> &Self {
            self.resources.push(ResourceFixture {
                name: name.to_string(),
                namespace: Some(namespace.to_string()),
                kind: "SecretClaim".to_string(),
                provider_url: None,
                data: Some(data),
            });
            self
        }

        pub async fn build(&self) {
            for resource in &self.resources {
                let yaml = match resource.kind.as_str() {
                    "ConfigurationStore" | "ClusterConfigurationStore" => format!(r#"
                        apiVersion: external-config.com/v1alpha1
                        kind: {}
                        metadata:
                          name: {}
                          {}
                        spec:
                          provider:
                            http:
                              url: {}
                    "#,
                      resource.kind, resource.name,
                      resource.namespace.as_ref().map(|ns| format!("namespace: {}", ns)).unwrap_or_default(),
                      resource.provider_url.as_ref().unwrap()),

                    "ConfigMapClaim" => {
                        let data_yaml = resource.data.as_ref()
                            .map(|data| {
                                data.iter()
                                    .map(|(k, v)| format!("    {}: {}", k, v)) // Use 4 spaces for each line in the `data` section
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                            .unwrap_or_default();

                        Self::format_config_map_claim(resource, data_yaml)
                    },
                    "SecretClaim" => {
                        let data_yaml = resource.data.as_ref()
                            .map(|data| {
                                data.iter()
                                    .map(|(k, v)| format!("    {}: {}", k, v)) // Use 4 spaces for each line in the `data` section
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                            .unwrap_or_default();

                        Self::format_secret_claim(resource, data_yaml)
                    },
                    _ => continue,
                };
                apply_from_yaml(self.client.clone(), &yaml).await.unwrap();
            }
        }

        fn format_config_map_claim(resource: &ResourceFixture, data_yaml: String) -> String {
            format!(r#"
apiVersion: external-config.com/v1alpha1
kind: ConfigMapClaim
metadata:
  name: {}
  namespace: {}
spec:
  data:
{}

  target:
    creationPolicy: Owned
    name: {}

  refreshInterval: 5m
"#, resource.name, resource.namespace.as_ref().unwrap(), data_yaml, resource.name)
        }
        fn format_secret_claim(resource: &ResourceFixture, data_yaml: String) -> String {
            format!(r#"
apiVersion: external-config.com/v1alpha1
kind: SecretClaim
metadata:
  name: {}
  namespace: {}
spec:
  data:
{}
  target:
    creationPolicy: Owned
    name: {}

  refreshInterval: 5m
"#, resource.name, resource.namespace.as_ref().unwrap(), data_yaml, resource.name)
        }

        pub async fn prepare_single_config_store_claim_scenario(&mut self, prefix: &str, configs: Vec<MockConfig>) {

            let mut cmcData =  HashMap::new();
            let formats = vec!["json", "yaml", "toml", "properties", "env"];

            for format in formats {
                cmcData.insert(format!("config.{}", format), String::from(format!(r#"
                  from:
                    - configurationStoreRef:
                        kind: ConfigurationStore
                        name: {}-store
                      configurationStoreParams:
                        testParam1: asd
                        testParam2: asd

                  strategy: Merge
            "#, prefix)));
            }



            let namespace = "default";
            self.add_configuration_store(
                format!("{}-store", prefix).as_str(), namespace, configs
            ).await;

            self.add_config_map_claim(
                format!("{}-cmc", prefix).as_str(), namespace, cmcData.clone()
            );

            self.add_secret_claim(
                format!("{}-sc", prefix).as_str(), namespace, cmcData.clone()
            )
            .build().await


        }

        pub async fn prepare_config_store_claim_with_merge_scenario(&mut self, prefix: &str, configs: Vec<MockConfig>) {

            let mut cmcData =  HashMap::new();
            let formats = vec!["json", "yaml", "toml", "properties", "env"];

            for format in formats {
                cmcData.insert(format!("config.{}", format), String::from(format!(r#"
                  from:
                    - configurationStoreRef:
                        kind: ConfigurationStore
                        name: {}-store
                      configurationStoreParams:
                        env: local
                    - configurationStoreRef:
                        kind: ConfigurationStore
                        name: {}-store
                      configurationStoreParams:
                        env: dev
                    - configurationStoreRef:
                        kind: ConfigurationStore
                        name: {}-store
                      configurationStoreParams:
                        env: prod


                  strategy: Merge
            "#, prefix, prefix, prefix)));
            }



            let namespace = "default";
            self.add_configuration_store(
                format!("{}-store", prefix).as_str(), namespace, configs
            ).await;

            self.add_config_map_claim(
                format!("{}-cmc", prefix).as_str(), namespace, cmcData.clone()
            );

            self.add_secret_claim(
                format!("{}-sc", prefix).as_str(), namespace, cmcData.clone()
            )
            .build().await


        }


        pub async fn prepare_single_cluster_config_store_claim_scenario(&mut self, prefix: &str, configs: Vec<MockConfig>) {

            let mut cmcData =  HashMap::new();
            let formats = vec!["json", "yaml", "toml", "properties", "env"];

            for format in formats {
                cmcData.insert(format!("config.{}", format), String::from(format!(r#"
                  from:
                    - configurationStoreRef:
                        kind: ClusterConfigurationStore
                        name: {}-store
                      configurationStoreParams:
                        testParam1: asd
                        testParam2: asd

                  strategy: Merge
            "#, prefix)));
            }



            let namespace = "default";
            self.add_cluster_configuration_store(
                format!("{}-store", prefix).as_str(), configs
            ).await;

            self.add_config_map_claim(
                format!("{}-cmc", prefix).as_str(), namespace, cmcData.clone()
            );

            self.add_secret_claim(
                format!("{}-sc", prefix).as_str(), namespace, cmcData.clone()
            )
                .build().await


        }

        pub async fn prepare_cluster_config_store_claim_with_merge_scenario(&mut self, prefix: &str, configs: Vec<MockConfig>) {

            let mut cmcData =  HashMap::new();
            let formats = vec!["json", "yaml", "toml", "properties", "env"];

            for format in formats {
                cmcData.insert(format!("config.{}", format), String::from(format!(r#"
                  from:
                    - configurationStoreRef:
                        kind: ClusterConfigurationStore
                        name: {}-store
                      configurationStoreParams:
                        env: local
                    - configurationStoreRef:
                        kind: ClusterConfigurationStore
                        name: {}-store
                      configurationStoreParams:
                        env: dev
                    - configurationStoreRef:
                        kind: ClusterConfigurationStore
                        name: {}-store
                      configurationStoreParams:
                        env: prod


                  strategy: Merge
            "#, prefix, prefix, prefix)));
            }



            let namespace = "default";
            self.add_cluster_configuration_store(
                format!("{}-store", prefix).as_str(), configs
            ).await;

            self.add_config_map_claim(
                format!("{}-cmc", prefix).as_str(), namespace, cmcData.clone()
            );

            self.add_secret_claim(
                format!("{}-sc", prefix).as_str(), namespace, cmcData.clone()
            )
                .build().await


        }

    }
}