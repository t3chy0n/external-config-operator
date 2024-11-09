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

    //Workaround to log from test run
    #[cfg(not(test))]
    use log::{info, warn};

    #[cfg(test)]
    use std::{println as info, println as warn};
    use kube::api::ListParams;
    ////

    use log::error;
    use tokio::task::JoinError;
    use crate::controller::controller::{apply_all_crds, apply_from_yaml};
    use crate::controller::v1alpha1::controller::ConfigurationStore;

    // Global array of Kubernetes versions
    const K8S_VERSIONS: &[&str] = &[
        "v1.29.10-k3s1",
        "v1.30.6-k3s1",
        "v1.31.2-k3s1",
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
            }
        }
        let config = Config::from_custom_kubeconfig(kubeconfig, &Default::default()).await.unwrap();
        Arc::new(Client::try_from(config).expect("Could not establish connection to cluster"))
    }


    #[tokio::test]
    async fn run_test_body() {

        // setup_k3s_containers().await;

        for version in K8S_VERSIONS {

            let container = get_k8s_container(version).await;
            let client = get_k8s_client(version, &container).await;

            apply_all_crds(&client).await.expect("Could not apply crds to test cluster");

            let res = test_crd_deployment(client.clone()).await.unwrap();
            // Here, you'd add your test code, using `version` and `container`
            println!("Running test on Kubernetes version: {}", version);

            // Clean up container
            //drop(container);

            println!("Test on Kubernetes version {} completed successfully.", version);
        }
        // Set up the container for the specific Kubernetes version

    }

    async fn test_crd_deployment(client: Arc<Client>) -> Result<String, Error> {

        // Verify `ClusterConfigurationStore` CRD is deployed
        let config_store_api = Api::<ConfigurationStore>::namespaced((*client).clone(), "default");
        let store_ref = ClaimConfigurationStoreRef {
            name: "test-store".to_string(),
            kind: SupportedConfigurationStoreResourceType::ClusterConfigurationStore,
        };

//         let manifest = r#"
// apiVersion: external-config.com/v1alpha1
// kind: ClusterConfigurationStore
// metadata:
//   name: test-store
// spec:
//   provider:
//     http:
//       url: https://raw.githubusercontent.com/Ylianst/MeshCentral/refs/heads/master/sample-config.json
//
// "#;
        let manifest = r#"
apiVersion: external-config.com/v1alpha1
kind: ConfigurationStore
metadata:
  name: test-store
  namespace: default
spec:
  provider:
    http:
      url: https://raw.githubusercontent.com/Ylianst/MeshCentral/refs/heads/master/sample-config.json

"#;
        apply_from_yaml(client, manifest).await.expect("Could not deploy test manifest");

        // Attempt to get the CRD instance and confirm structure
        let all = config_store_api.list(&ListParams::default().limit(10)).await.map_err(Error::KubeError)?;
        let store = config_store_api.get(&store_ref.name).await.map_err(Error::KubeError)?;
        let config_store = store.spec.provider.get_config_store();
        let file = "test-config"; // Update this as needed for your test case
        let config = config_store.get_config(file.to_string()).await;

        assert_eq!(config.is_err(), false);
        config

    }

    macro_rules! define_k8s_tests {
        // Match the test functions followed by the client function
        ({ $($subtest:ident),+ $(,)? }, $client_fn:ident) => {
            #[tokio::test]
            async fn run_tests() {
                let mut version_suite_tasks = vec![];
                for version in K8S_VERSIONS {


                    version_suite_tasks.push(tokio::task::spawn(async move {
                        let container = get_k8s_container(version).await;
                        let client = $client_fn(version, &container).await;

                        let mut tasks = vec![];
                        $(
                            // Run each subtest as an async block
                            let test_name = format!("{} - {}", stringify!($subtest), version);
                            let cloned_client = client.clone();
                            tasks.push(tokio::task::spawn(async move {
                                info!("{}", format!("Running {}...", test_name).yellow().bold());
                                let result = $subtest(cloned_client).await;
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

    async fn test_other_feature(client: Arc<Client>) -> Result<String, Error> {
        // Example subtest logic here
        // ...
        Ok("Other Feature Test successful".to_string())
    }

    async fn test_other_feature2(client: Arc<Client>) -> Result<String, Error> {
        // Example subtest logic here
        // ...
        Ok("Other2 Feature Test successful".to_string())
    }

    async fn test_other_feature3(client: Arc<Client>) -> Result<String, Error> {
        // Example subtest logic here
        // ...
        Ok("Other2 Feature Test successful".to_string())
    }

    // Additional subtest functions can be defined here

    // Use the macro to generate multiple test sets with grouped subtests
    define_k8s_tests!({
            // Test name `run_crd_tests` with its client function and subtests
            // test_crd_deployment,
            // Test name `run_feature_tests` with its client function and different subtests
           test_other_feature,
           test_other_feature2,
           test_other_feature3,
        },
        get_k8s_client
    );

}