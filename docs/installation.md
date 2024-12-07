

## Installation

### Prerequisites
- Kubernetes cluster (v1.20 or higher recommended).
- Helm (v3 or higher).
- Permissions to create CRDs, Deployments, and associated Kubernetes resources.

### Deploying the Operator with Helm
The operator uses a Helm chart for installation, enabling customization of deployment settings.

1. **Add the Helm Repository**:

   ```bash
   helm repo add external-config-operator https://example.com/helm-charts
   helm repo update

2. **Install the Chart**:
```bash
helm install external-config-operator external-config-operator/external-config-operator \
  --set image.repository=your-docker-repo/operator-image \
  --set image.tag=latest
  --set installCrd=true
```

3. **Verify Installation:**:
```bash
kubectl get deployments -n <namespace>
kubectl logs deployment/external-config-operator -n <namespace>

helm test external-config-operator
```
