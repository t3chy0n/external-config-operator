#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::env::temp_dir;
    use std::sync::{Arc};
    use kube::{Api, Client, Config};
    use kube::config::Kubeconfig;
    use testcontainers::{ContainerAsync, GenericImage, ImageExt, TestcontainersError};
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::{k3s::K3s};
    use colored::*;
    use futures::future::join_all;
    use crate::contract::lib::Error;
    use crate::controller::v1alpha1::crd::claim::{ClaimConfigurationStoreRef, SupportedConfigurationStoreResourceType};
    use crate::controller::v1alpha1::crd::configuration_store::ClusterConfigurationStore;
    use crate::contract::clients::K8sClientAware;

    //Workaround to log from test run
    #[cfg(not(test))]
    use log::{info, warn};

    #[cfg(test)]
    use std::{println as info, println as warn};
    use std::ffi::c_double;
    use base64::Engine;
    use kube::api::{ListParams, ObjectList};
    use kube::runtime::Controller;
    ////

    use log::error;
    use tokio::task::JoinError;
    use crate::contract::ireconcilable::IReconcilable;
    use crate::controller::controller::{apply_all_crds, apply_from_yaml, reconcile};
    use crate::controller::utils::context::Data;
    use crate::controller::v1alpha1::controller::{ConfigMapClaim, ConfigurationStore};
    use crate::controller::v1alpha1::crd_client::{CrdClient};
    use crate::controller::v1alpha1::fixtures::tests::{ControllerFixtures, MockConfig};
    use base64::engine::general_purpose::STANDARD;
    use k8s_openapi::ByteString;
    use crate::contract::clients::ICrdClient;
    use crate::contract::clients::K8sClient;

    // Global array of Kubernetes versions
    const K8S_VERSIONS: &[&str] = &[
        // "v1.29.10-k3s1",
        // "v1.30.6-k3s1",
        // "v1.31.2-k3s1",
        "latest",
    ];

    type ContainerMap = HashMap<String, Arc<ContainerAsync<K3s>>>;

    async fn get_k8s_container(version: &str) -> ContainerAsync<K3s> {
        let conf_dir = temp_dir();


        K3s::default()
            .with_conf_mount(&conf_dir)
            .with_tag(version)
            .with_privileged(true)
            .start()
            .await.unwrap()
    }

    async fn get_k8s_client(version: &str, container: &ContainerAsync<K3s>) -> Arc<Client> {

        let kubeconfig_yaml = container.image().read_kube_config().unwrap();
        let mut kubeconfig = Kubeconfig::from_yaml(&kubeconfig_yaml).unwrap();
        // Retrieve the IP address and port of the K3s container
        let container_port = container.get_host_port_ipv4(6443).await.expect("Unable to get container IP");

        // Update the server field in kubeconfig with the container's IP and port
        for cluster in &mut kubeconfig.clusters {
            if let Some(cluster_details) = &mut cluster.cluster {
                cluster_details.server = Some(format!("https://localhost:{}", container_port));

            } else {
                println!("No cluster configuration")
            }
        }
        println!("Config details: {:?}", kubeconfig);
        let config = Config::from_custom_kubeconfig(kubeconfig, &Default::default()).await.unwrap();
        Arc::new(Client::try_from(config).expect("Could not establish connection to cluster"))
    }



    // #[tokio::test]
    // async fn run_test_body() {
    //
    //     // setup_k3s_containers().await;
    //
    //     for version in K8S_VERSIONS {
    //
    //         let container = get_k8s_container(version).await;
    //         let client = get_k8s_client(version, &container).await;
    //         let crd_client = CrdClient::new(client.clone());
    //         let mut fixture = ControllerFixtures::new(client.clone()).await;
    //
    //         apply_all_crds(client).await.expect("Could not apply crds to test cluster");
    //
    //         let res = test_crd_deployment(crd_client, &mut fixture).await.unwrap();
    //         // Here, you'd add your test code, using `version` and `container`
    //         println!("Running test on Kubernetes version: {}", version);
    //
    //         // Clean up container
    //         //drop(container);
    //
    //         println!("Test on Kubernetes version {} completed successfully.", version);
    //     }
    //     // Set up the container for the specific Kubernetes version
    //
    // }


    macro_rules! define_k8s_tests {
        // Match the test functions followed by the client function
        ({ $($subtest:ident),+ $(,)? }) => {
            #[tokio::test]
            async fn run_tests() {
                let mut version_suite_tasks = vec![];
                for version in K8S_VERSIONS {
                    let container = get_k8s_container(version).await;
                    version_suite_tasks.push(tokio::task::spawn(async move {

                        let client = get_k8s_client(version, &container).await;

                        apply_all_crds(client.clone()).await.expect("Could not apply crds to test cluster");

                        let mut tasks = vec![];
                        $(
                            // Run each subtest as an async block
                            let test_name = format!("{} - {}", stringify!($subtest), version);
                            let crd_client = Arc::new(CrdClient::new(client.clone()));
                            let mut fixture = ControllerFixtures::new(client.clone()).await;

                            let context = Arc::new(Data{
                                client: client.clone(),
                                v1alpha1: crd_client.clone(),
                                api_client: crd_client.clone()
                            });
                            let cloned_client = client.clone();
                            tasks.push(tokio::task::spawn(async move {
                                info!("{}", format!("Running {}...", test_name).yellow().bold());

                                let result = $subtest(context, &mut fixture).await;
                                if let Err(err) = result {
                                    let err_msg = format!("{} failed: {}", test_name, err);
                                    eprintln!("{}", err_msg.red().bold());
                                    panic!("test_name failed: {:?}", err_msg);
                                } else {
                                    eprintln!("{}", format!("{} completed successfully.", test_name).green().bold());
                                }
                            }));
                        )+

                            // Await all subtests and collect results
                        let results: Vec<Result< (), JoinError>> = join_all(tasks).await
                            .into_iter()

                            .collect();
                        //Result<Result<(), std::string::String>>

                        // Return an error if any subtest failed
                        for result in results {
                           if let Err(err) = result {
                                panic!("run_tests failed: {:?}", err); // Ensures `run_tests` fails if any subtest failed
                            }
                            if result.is_err() {
                                return Err("One or more subtests failed".into());
                            }
                        }
                        Ok::<(), String>(())

                    }));

                }

                // Await all version tasks and propagate any errors
                let version_results = join_all(version_suite_tasks).await;
                for result in version_results {
                    if let Err(err) = result {
                        panic!("run_tests failed: {}", err); // Ensures `run_tests` fails if any subtest failed
                    }
                }
            }

        };
    }


    async fn test_successful_config_store_data_resolution(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {

        // Verify `ClusterConfigurationStore` CRD is deployed
        let store_name = "test-crd-deployment";
        let namespace = "default";

        fixture.prepare_single_config_store_claim_scenario(store_name, vec![MockConfig::success_with_body("{\"asd\": 1}")]).await;

        // Attempt to get the CRD instance and confirm structure
        let store = ctx.v1alpha1.get_config_store(format!("{}-store",store_name).as_str(), namespace).await?;

        let config_store = store.spec.provider.get_config_store();

        let config = config_store.get_config(None, None).await.expect("Config should be returned");

        assert_eq!(config, "{\"asd\": 1}");

        Ok(config)

    }
    async fn test_successful_config_store_common_parameter(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {

        // Verify `ClusterConfigurationStore` CRD is deployed
        let store_name = "test-crd-deployment-config-store-common-parameter";
        let namespace = "default";

        let mut query_params = HashMap::new();
        query_params.insert(String::from("store_param"), String::from("test"));

        fixture.prepare_single_config_store_claim_scenario(store_name, vec![MockConfig::query_params_response(query_params, "{\"test\": 1}")]).await;

        // Attempt to get the CRD instance and confirm structure
        let store = ctx.v1alpha1.get_config_store(format!("{}-store",store_name).as_str(), namespace).await?;

        let config_store = store.spec.provider.get_config_store();

        let config = config_store.get_config(None, None).await.expect("Config should be returned");

        assert_eq!(config, "{\"test\": 1}");

        Ok(config)

    }
    async fn test_successful_cluster_config_store_data_resolution(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {

        // Verify `ClusterConfigurationStore` CRD is deployed
        let store_name = "test-crd-deployment-cluster-config-store";
        let namespace = "default";

        fixture.prepare_single_cluster_config_store_claim_scenario(store_name, vec![MockConfig::success_with_body("{\"asd\": 1}")]).await;

        // Attempt to get the CRD instance and confirm structure
        let store = ctx.v1alpha1.get_cluster_config_store(format!("{}-store",store_name).as_str()).await?;

        let config_store = store.spec.provider.get_config_store();

        let config = config_store.get_config(None, None).await.expect("Config should be returned");

        assert_eq!(config, "{\"asd\": 1}");

        Ok(config)

    }

    async fn test_client_error_config_store_data_resolution(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {

        // Verify `ClusterConfigurationStore` CRD is deployed
        let store_name = "test-error-config-store-data-resolution";
        let namespace = "default";

        fixture.prepare_single_config_store_claim_scenario(store_name, vec![MockConfig::not_found_with_body("{\"error\": \"Not found\"}")]).await;

        // Attempt to get the CRD instance and confirm structure
        let store = ctx.v1alpha1.get_config_store(format!("{}-store",store_name).as_str(), namespace).await?;

        let config_store = store.spec.provider.get_config_store();

        let config = config_store.get_config(None, None).await;


        match config {
            Err(Error::HttpConfigStoreClientError(err)) => {
                // Check that the error message matches the expected value
                let error_message = format!("{}", err);
                assert_eq!(error_message, "{\"error\": \"Not found\"}");
            },
            _ => panic!("Expected Error::HttpConfigStoreClientError but got a different error"),
        }
        Ok(String::from("Done"))

    }
    async fn test_server_error_config_store_data_resolution(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {

        // Verify `ClusterConfigurationStore` CRD is deployed
        let store_name = "test-server-error-config-store-data-resolution";
        let namespace = "default";

        fixture.prepare_single_config_store_claim_scenario(store_name, vec![MockConfig::internal_server_error_with_body("{\"error\": \"Internal server error\"}")]).await;

        // Attempt to get the CRD instance and confirm structure
        let store = ctx.v1alpha1.get_config_store(format!("{}-store",store_name).as_str(), namespace).await?;

        let config_store = store.spec.provider.get_config_store();

        let config = config_store.get_config(None, None).await;


        match config {
            Err(Error::HttpConfigStoreServerError(kube_error)) => {
                // Check that the error message matches the expected value
                let error_message = format!("{}", kube_error);
                assert_eq!(error_message, "{\"error\": \"Internal server error\"}");
            },
            _ => panic!("Expected Error::HttpConfigStoreServerError but got a different error"),
        }
        Ok(String::from("Done"))

    }
    async fn test_config_store_returns_data_depending_on_params(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {

        // Verify `ClusterConfigurationStore` CRD is deployed
        let store_name = "test-server-error-config-store-data-resolution-based-on-params";
        let namespace = "default";

        fixture.prepare_single_config_store_claim_scenario(store_name,
   vec![
            MockConfig::query_param_response("test1", "value1", "Some"),
            MockConfig::query_param_response("test1", "value2", "Some2")
        ]).await;

        // Attempt to get the CRD instance and confirm structure
        let store = ctx.v1alpha1.get_config_store(format!("{}-store",store_name).as_str(), namespace).await?;

        let config_store = store.spec.provider.get_config_store();

        let mut params1: HashMap<String, String> = HashMap::new();
        params1.insert(String::from("test1"), String::from("value1"));
        let config1 = config_store.get_config(Some(params1), None).await.expect("Configuration based on test1 param couldn't be retrieved");

        let mut params2 = HashMap::new();
        params2.insert(String::from("test1"), String::from("value2"));
        let config2 = config_store.get_config(Some(params2), None).await.expect("Configuration based on test1 param couldn't be retrieved");

        assert_eq!(config1, "Some");
        assert_eq!(config2, "Some2");

        Ok(String::from("Done"))

    }


    async fn test_basic_reconcilation(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {

        // Verify `ClusterConfigurationStore` CRD is deployed
        let store_name = "test-basic-reconcilation";
        let namespace = "default";

        fixture.prepare_single_config_store_claim_scenario(store_name, vec![MockConfig::success_with_body("{\"dbConfig\": { \"host\": \"test_host\", logLevels: [\"INFO\", \"DEBUG\"]} }")]).await;

        // Attempt to get the CRD instance and confirm structure
        let claim = ctx.v1alpha1.get_config_map_claim(format!("{}-cmc",store_name).as_str(), namespace).await.expect("Config Map Claim could not be found");
        let secret_claim = ctx.v1alpha1.get_secret_claim(format!("{}-sc",store_name).as_str(), namespace).await.expect("Secret Claim could not be found");

        let reconsile_action = claim.reconcile( ctx.clone()).await?;
        let reconsile_action_secret = secret_claim.reconcile( ctx.clone()).await?;

        let mut config_map = ctx.v1alpha1.get_config_map(format!("{}-cmc",store_name).as_str(), namespace).await.expect("Config map was not reconciled properly");
        let mut secret_map = ctx.v1alpha1.get_secret(format!("{}-sc",store_name).as_str(), namespace).await.expect("Secret was not reconciled properly");

        let cm_data = config_map.data.unwrap();
        let sm_data = secret_map.data.unwrap();

        let data = String::from(cm_data.get("config.yaml").unwrap().as_str());
        let data1 = String::from(cm_data.get("config.json").unwrap().as_str());
        let data2 = String::from(cm_data.get("config.toml").unwrap().as_str());
        let data3 = String::from(cm_data.get("config.properties").unwrap().as_str());
        let data4 = String::from(cm_data.get("config.env").unwrap().as_str());

        let str = sm_data.get("config.yaml").unwrap();
        let str1 = sm_data.get("config.json").unwrap();
        let str2 = sm_data.get("config.toml").unwrap();
        let str3 = sm_data.get("config.properties").unwrap();
        let str4 = sm_data.get("config.env").unwrap();

        let sdata = String::from_utf8(STANDARD.decode(&str.0).unwrap()).unwrap();
        let sdata1 = String::from_utf8(STANDARD.decode(&str1.0).unwrap()).unwrap();
        let sdata2 = String::from_utf8(STANDARD.decode(&str2.0).unwrap()).unwrap();
        let sdata3 = String::from_utf8(STANDARD.decode(&str3.0).unwrap()).unwrap();
        let sdata4 = String::from_utf8(STANDARD.decode(&str4.0).unwrap()).unwrap();

        assert_eq!(data, sdata);
        assert_eq!(data1, sdata1);
        assert_eq!(data2, sdata2);
        assert_eq!(data3, sdata3);
        assert_eq!(data4, sdata4);

        assert_eq!(data, r#"dbConfig:
  host: test_host
  logLevels:
  - INFO
  - DEBUG
"#);
        assert_eq!(data1, r#"{
  "dbConfig": {
    "host": "test_host",
    "logLevels": [
      "INFO",
      "DEBUG"
    ]
  }
}"#);
        assert_eq!(data2, r#"[dbConfig]
host = "test_host"
logLevels = ["INFO", "DEBUG"]
"#);
        assert_eq!(data3, r#"dbConfig.host=test_host
dbConfig.logLevels.0=INFO
dbConfig.logLevels.1=DEBUG"#);
        assert_eq!(data4, r#"DB__CONFIG_HOST=test_host
DB__CONFIG_LOG__LEVELS_0=INFO
DB__CONFIG_LOG__LEVELS_1=DEBUG"#);
        Ok(String::from("Done"))

    }

    async fn test_config_files_with_merging_reconcilation(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {

        // Verify `ClusterConfigurationStore` CRD is deployed
        let store_name = "test-config-files-with-merging-reconcilation";
        let namespace = "default";

        fixture.prepare_config_store_claim_with_merge_scenario(store_name, vec![
            MockConfig::query_param_response("env", "local", "{\"dbConfig\": { \"host\": \"localhost\", \"logLevels\": [\"INFO\",  \"ERROR\",  \"DEBUG\"], \"timeout\": 1000} }"),
            MockConfig::query_param_response("env", "dev", "{\"dbConfig\": { \"host\": \"dev\", \"logLevels\": [\"INFO\", \"DEBUG\"], \"logApiKey\": \"logger_key\"} }"),
            MockConfig::query_param_response("env", "prod", "{\"dbConfig\": { \"host\": \"prod\", \"logLevels\": [\"INFO\"]} }"),
        ]).await;

        // Attempt to get the CRD instance and confirm structure
        let claim = ctx.v1alpha1.get_config_map_claim(format!("{}-cmc",store_name).as_str(), namespace).await.expect("Config Map Claim could not be found");
        let secret_claim = ctx.v1alpha1.get_secret_claim(format!("{}-sc",store_name).as_str(), namespace).await.expect("Secret Claim could not be found");

        let reconsile_action = claim.reconcile( ctx.clone()).await?;
        let reconsile_action_secret = secret_claim.reconcile( ctx.clone()).await?;

        let mut config_map = ctx.v1alpha1.get_config_map(format!("{}-cmc",store_name).as_str(), namespace).await.expect("Config map was not reconciled properly");
        let mut secret_map = ctx.v1alpha1.get_secret(format!("{}-sc",store_name).as_str(), namespace).await.expect("Secret was not reconciled properly");

        let cm_data = config_map.data.unwrap();
        let sm_data = secret_map.data.unwrap();

        let data = String::from(cm_data.get("config.yaml").unwrap().as_str());
        let data1 = String::from(cm_data.get("config.json").unwrap().as_str());
        let data2 = String::from(cm_data.get("config.toml").unwrap().as_str());
        let data3 = String::from(cm_data.get("config.properties").unwrap().as_str());
        let data4 = String::from(cm_data.get("config.env").unwrap().as_str());

        let str = sm_data.get("config.yaml").unwrap();
        let str1 = sm_data.get("config.json").unwrap();
        let str2 = sm_data.get("config.toml").unwrap();
        let str3 = sm_data.get("config.properties").unwrap();
        let str4 = sm_data.get("config.env").unwrap();

        let sdata = String::from_utf8(STANDARD.decode(&str.0).unwrap()).unwrap();
        let sdata1 = String::from_utf8(STANDARD.decode(&str1.0).unwrap()).unwrap();
        let sdata2 = String::from_utf8(STANDARD.decode(&str2.0).unwrap()).unwrap();
        let sdata3 = String::from_utf8(STANDARD.decode(&str3.0).unwrap()).unwrap();
        let sdata4 = String::from_utf8(STANDARD.decode(&str4.0).unwrap()).unwrap();

        assert_eq!(data, sdata);
        assert_eq!(data1, sdata1);
        assert_eq!(data2, sdata2);
        assert_eq!(data3, sdata3);
        assert_eq!(data4, sdata4);

        assert_eq!(data, r#"dbConfig:
  host: prod
  logApiKey: logger_key
  logLevels:
  - INFO
  timeout: 1000
"#);
        assert_eq!(data1, r#"{
  "dbConfig": {
    "host": "prod",
    "logApiKey": "logger_key",
    "logLevels": [
      "INFO"
    ],
    "timeout": 1000
  }
}"#);
        assert_eq!(data2, r#"[dbConfig]
host = "prod"
logApiKey = "logger_key"
logLevels = ["INFO"]
timeout = 1000
"#);
        assert_eq!(data3, r#"dbConfig.host=prod
dbConfig.logApiKey=logger_key
dbConfig.logLevels.0=INFO
dbConfig.timeout=1000"#);
        assert_eq!(data4, r#"DB__CONFIG_HOST=prod
DB__CONFIG_LOG__API__KEY=logger_key
DB__CONFIG_LOG__LEVELS_0=INFO
DB__CONFIG_TIMEOUT=1000"#);
        Ok(String::from("Done"))

    }

    async fn test_basic_reconcilation_with_cluster_config_store(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {

        // Verify `ClusterConfigurationStore` CRD is deployed
        let store_name = "test-basic-reconcilation-with-cluster-config-store";
        let namespace = "default";

        fixture.prepare_single_cluster_config_store_claim_scenario(store_name, vec![MockConfig::success_with_body("{\"dbConfig\": { \"host\": \"test_host\", logLevels: [\"INFO\", \"DEBUG\"]} }")]).await;

        // Attempt to get the CRD instance and confirm structure
        let claim = ctx.v1alpha1.get_config_map_claim(format!("{}-cmc",store_name).as_str(), namespace).await.expect("Config Map Claim could not be found");
        let secret_claim = ctx.v1alpha1.get_secret_claim(format!("{}-sc",store_name).as_str(), namespace).await.expect("Secret Claim could not be found");

        let reconsile_action = claim.reconcile( ctx.clone()).await?;
        let reconsile_action_secret = secret_claim.reconcile(ctx.clone()).await?;

        let mut config_map = ctx.v1alpha1.get_config_map(format!("{}-cmc",store_name).as_str(), namespace).await.expect("Config map was not reconciled properly");
        let mut secret_map = ctx.v1alpha1.get_secret(format!("{}-sc",store_name).as_str(), namespace).await.expect("Secret was not reconciled properly");

        let cm_data = config_map.data.unwrap();
        let sm_data = secret_map.data.unwrap();

        let data = String::from(cm_data.get("config.yaml").unwrap().as_str());
        let data1 = String::from(cm_data.get("config.json").unwrap().as_str());
        let data2 = String::from(cm_data.get("config.toml").unwrap().as_str());
        let data3 = String::from(cm_data.get("config.properties").unwrap().as_str());
        let data4 = String::from(cm_data.get("config.env").unwrap().as_str());

        let str = sm_data.get("config.yaml").unwrap();
        let str1 = sm_data.get("config.json").unwrap();
        let str2 = sm_data.get("config.toml").unwrap();
        let str3 = sm_data.get("config.properties").unwrap();
        let str4 = sm_data.get("config.env").unwrap();

        let sdata = String::from_utf8(STANDARD.decode(&str.0).unwrap()).unwrap();
        let sdata1 = String::from_utf8(STANDARD.decode(&str1.0).unwrap()).unwrap();
        let sdata2 = String::from_utf8(STANDARD.decode(&str2.0).unwrap()).unwrap();
        let sdata3 = String::from_utf8(STANDARD.decode(&str3.0).unwrap()).unwrap();
        let sdata4 = String::from_utf8(STANDARD.decode(&str4.0).unwrap()).unwrap();

        assert_eq!(data, sdata);
        assert_eq!(data1, sdata1);
        assert_eq!(data2, sdata2);
        assert_eq!(data3, sdata3);
        assert_eq!(data4, sdata4);

        assert_eq!(data, r#"dbConfig:
  host: test_host
  logLevels:
  - INFO
  - DEBUG
"#);
        assert_eq!(data1, r#"{
  "dbConfig": {
    "host": "test_host",
    "logLevels": [
      "INFO",
      "DEBUG"
    ]
  }
}"#);
        assert_eq!(data2, r#"[dbConfig]
host = "test_host"
logLevels = ["INFO", "DEBUG"]
"#);
        assert_eq!(data3, r#"dbConfig.host=test_host
dbConfig.logLevels.0=INFO
dbConfig.logLevels.1=DEBUG"#);
        assert_eq!(data4, r#"DB__CONFIG_HOST=test_host
DB__CONFIG_LOG__LEVELS_0=INFO
DB__CONFIG_LOG__LEVELS_1=DEBUG"#);
        Ok(String::from("Done"))

    }

    async fn test_config_files_with_merging_reconcilation_with_cluster_config_store(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {

        // Verify `ClusterConfigurationStore` CRD is deployed
        let store_name = "test-config-files-with-merging-reconcilation-with-cluster-config-store";
        let namespace = "default";

        fixture.prepare_cluster_config_store_claim_with_merge_scenario(store_name, vec![
            MockConfig::query_param_response("env", "local", "{\"dbConfig\": { \"host\": \"localhost\", \"logLevels\": [\"INFO\",  \"ERROR\",  \"DEBUG\"], \"timeout\": 1000} }"),
            MockConfig::query_param_response("env", "dev", "{\"dbConfig\": { \"host\": \"dev\", \"logLevels\": [\"INFO\", \"DEBUG\"], \"logApiKey\": \"logger_key\"} }"),
            MockConfig::query_param_response("env", "prod", "{\"dbConfig\": { \"host\": \"prod\", \"logLevels\": [\"INFO\"]} }"),
        ]).await;

        // Attempt to get the CRD instance and confirm structure
        let claim = ctx.v1alpha1.get_config_map_claim(format!("{}-cmc",store_name).as_str(), namespace).await.expect("Config Map Claim could not be found");
        let secret_claim = ctx.v1alpha1.get_secret_claim(format!("{}-sc",store_name).as_str(), namespace).await.expect("Secret Claim could not be found");

        let reconsile_action = claim.reconcile( ctx.clone()).await?;
        let reconsile_action_secret = secret_claim.reconcile( ctx.clone()).await?;

        let mut config_map = ctx.v1alpha1.get_config_map(format!("{}-cmc",store_name).as_str(), namespace).await.expect("Config map was not reconciled properly");
        let mut secret_map = ctx.v1alpha1.get_secret(format!("{}-sc",store_name).as_str(), namespace).await.expect("Secret was not reconciled properly");

        let cm_data = config_map.data.unwrap();
        let sm_data = secret_map.data.unwrap();

        let data = String::from(cm_data.get("config.yaml").unwrap().as_str());
        let data1 = String::from(cm_data.get("config.json").unwrap().as_str());
        let data2 = String::from(cm_data.get("config.toml").unwrap().as_str());
        let data3 = String::from(cm_data.get("config.properties").unwrap().as_str());
        let data4 = String::from(cm_data.get("config.env").unwrap().as_str());

        let str = sm_data.get("config.yaml").unwrap();
        let str1 = sm_data.get("config.json").unwrap();
        let str2 = sm_data.get("config.toml").unwrap();
        let str3 = sm_data.get("config.properties").unwrap();
        let str4 = sm_data.get("config.env").unwrap();

        let sdata = String::from_utf8(STANDARD.decode(&str.0).unwrap()).unwrap();
        let sdata1 = String::from_utf8(STANDARD.decode(&str1.0).unwrap()).unwrap();
        let sdata2 = String::from_utf8(STANDARD.decode(&str2.0).unwrap()).unwrap();
        let sdata3 = String::from_utf8(STANDARD.decode(&str3.0).unwrap()).unwrap();
        let sdata4 = String::from_utf8(STANDARD.decode(&str4.0).unwrap()).unwrap();

        assert_eq!(data, sdata);
        assert_eq!(data1, sdata1);
        assert_eq!(data2, sdata2);
        assert_eq!(data3, sdata3);
        assert_eq!(data4, sdata4);

        assert_eq!(data, r#"dbConfig:
  host: prod
  logApiKey: logger_key
  logLevels:
  - INFO
  timeout: 1000
"#);
        assert_eq!(data1, r#"{
  "dbConfig": {
    "host": "prod",
    "logApiKey": "logger_key",
    "logLevels": [
      "INFO"
    ],
    "timeout": 1000
  }
}"#);
        assert_eq!(data2, r#"[dbConfig]
host = "prod"
logApiKey = "logger_key"
logLevels = ["INFO"]
timeout = 1000
"#);
        assert_eq!(data3, r#"dbConfig.host=prod
dbConfig.logApiKey=logger_key
dbConfig.logLevels.0=INFO
dbConfig.timeout=1000"#);
        assert_eq!(data4, r#"DB__CONFIG_HOST=prod
DB__CONFIG_LOG__API__KEY=logger_key
DB__CONFIG_LOG__LEVELS_0=INFO
DB__CONFIG_TIMEOUT=1000"#);
        Ok(String::from("Done"))

    }
    async fn test_other_feature(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {
        // Example subtest logic here
        // ...
        Ok("Other Feature Test successful".to_string())
    }

    async fn test_other_feature2(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {
        // Example subtest logic here
        // ...
        Ok("Other2 Feature Test successful".to_string())
    }

    async fn test_other_feature3(ctx: Arc<Data>, fixture: &mut ControllerFixtures) -> Result<String, Error> {
        // Example subtest logic here
        // ...
        Ok("Other2 Feature Test successful".to_string())
    }

    // Additional subtest functions can be defined here
    // Use the macro to generate multiple test sets with grouped subtests
    define_k8s_tests!({
            test_successful_config_store_data_resolution,
            test_successful_cluster_config_store_data_resolution,

            test_successful_config_store_common_parameter,
            test_client_error_config_store_data_resolution,
            test_server_error_config_store_data_resolution,
            test_config_store_returns_data_depending_on_params,
            test_basic_reconcilation,
            test_config_files_with_merging_reconcilation,
            test_config_files_with_merging_reconcilation_with_cluster_config_store,
            test_basic_reconcilation_with_cluster_config_store,

           // test_other_feature,
           // test_other_feature2,
           // test_other_feature3,
        }
    );

}