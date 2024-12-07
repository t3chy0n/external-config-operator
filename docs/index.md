# User Manual for External Config Operator

## Overview
The External Config Operator is a Kubernetes operator designed to manage external configuration sources and integrate them seamlessly into Kubernetes clusters. It provides a streamlined mechanism for discovering and syncing configurations via an HTTP-based provider or other supported backends.

## Architecture overview:

```mermaid
graph TD
    subgraph Kubernetes Cluster

        MainController[Controller]
        subgraph Network Policy
            subgraph ClusterStore
                direction TB
                ClusterStorePod1[Cluster Store Pod 1]
                ClusterStorePod2[Cluster Store Pod 2]
                ClusterStoreService[Cluster Store Service]
            end
            subgraph ClusterConfigStore
                direction TB
                ConfigStorePod1[Cluster Config Store Pod 1]
                ConfigStorePod2[Cluster Config Store Pod 2]
                ConfigStoreService[Cluster Config Store Service]
            end
            NetworkPolicy[Network Policy<br>Allow traffic only from Main Controller]
    end
    end

    MainController -->|Http| ClusterStoreService
    MainController -->|Http| ConfigStoreService
    ClusterStoreService -->|Access to pods| ClusterStorePod1
    ClusterStoreService -->|Access to pods| ClusterStorePod2
    ConfigStoreService -->|Access to pods| ConfigStorePod1
    ConfigStoreService -->|Access to pods| ConfigStorePod2
    NetworkPolicy -->|Applies to| ClusterStore
    NetworkPolicy -->|Applies to| ClusterConfigStore
```