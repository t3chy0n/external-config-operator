

## Installation from local chart

### Prerequisites
- Kubernetes cluster (v1.20 or higher recommended).
- Helm (v3 or higher).
- Permissions to create CRDs, Deployments, and associated Kubernetes resources.

### Deploying the Operator with Helm
The operator uses a Helm chart for installation, enabling customization of deployment settings.
In order to do that from local chart you can simply run from root directory

1. **Add the Helm Repository**:

   ```bash
   helm install external-config-operator ./helm



2. **Verify Installation:**:
```bash
kubectl get deployments -n <namespace>
kubectl logs deployment/external-config-operator -n <namespace>

helm test external-config-operator
```
