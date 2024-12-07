

## Deploy backend stores with helm chart

### Prerequisites
- Helm (v3 or higher).
- Permissions to create CRDs, Deployments, and associated Kubernetes resources.

### Introduction
External config operator, is capable to extract configuration files from arbitrary http server.
This allows one to create own endpoint to extract that information easily that will be
syncronized to ConfigMap or Secret.

Helm chart has a way of properly isolating network traffic via NetworkPolicy, such so only
external config controller is capable of quering local containers that are responsible to extract
sensitive configuration data.


For that purpose one can follow below to deploy either namespaced configuration stores
or cluster configuration stores available through out entire cluster.

In order to use this approach, CRDS have to be deployed. 

```yaml

configurationStores:
  - name: sub1
    namespace: test
    replicaCount: 1
    image:
      repository: image_name
      tag: latest
      pullPolicy: "Always"
    containerPort: 8080
    servicePort: 80
    resources:
      limits:
        memory: "256Mi"
        cpu: "500m"
      requests:
        memory: "128Mi"
        cpu: "250m"
    env:
      LOG_LEVEL: "debug"
      ENV: "production"

clusterConfigurationStores:
  - name: sub3
    replicaCount: 1
    image:
      repository: image_name
      tag: latest
      pullPolicy: "Always"
    containerPort: 8080
    servicePort: 80
    resources:
      limits:
        memory: "256Mi"
        cpu: "500m"
      requests:
        memory: "128Mi"
        cpu: "250m"
    env:
      LOG_LEVEL: "debug"
      ENV: "production"

```

The above configuration will create seperate deployments for <image_name> pod(s), apply 
appropriate NetworkPolicies, create Services and finally create CRD for 
ConfigurationStore and ClusterConfigurationStore