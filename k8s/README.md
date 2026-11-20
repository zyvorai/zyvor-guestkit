# Guestkit Worker - Kubernetes Deployment

Deploy the guestkit distributed worker system to Kubernetes.

> **Production:** Use the Helm chart at [`deploy/helm/zyvor/`](../deploy/helm/zyvor/) for the full Zyvor VM Services stack (API, UI, Redis, PostgreSQL, MinIO). This DaemonSet is intended for local development and file-based job submission.

## Prerequisites

- Kubernetes cluster (k3d, minikube, or production cluster)
- kubectl configured
- Docker for building images

## Quick Start

### 1. Build Docker Image

```bash
# From the guestkit root directory
sudo docker build -f crates/guestkit-worker/Dockerfile -t guestkit-worker:latest .
```

### 2. Import Image to k3d (if using k3d)

```bash
sudo k3d image import guestkit-worker:latest -c hyper2kvm-test
```

### 3. Label Worker Nodes

```bash
# Label nodes where workers should run
sudo kubectl label nodes k3d-hyper2kvm-test-agent-0 guestkit.io/worker-enabled=true
sudo kubectl label nodes k3d-hyper2kvm-test-agent-1 guestkit.io/worker-enabled=true
```

### 4. Deploy

```bash
# Deploy all resources
sudo kubectl apply -k k8s/

# Or deploy individually
sudo kubectl apply -f k8s/namespace.yaml
sudo kubectl apply -f k8s/serviceaccount.yaml
sudo kubectl apply -f k8s/configmap.yaml
sudo kubectl apply -f k8s/daemonset.yaml
```

### 5. Verify Deployment

```bash
# Check pods
sudo kubectl get pods -n guestkit-workers

# Check logs
sudo kubectl logs -n guestkit-workers -l app=guestkit-worker --tail=20

# Check worker status
sudo kubectl get daemonset -n guestkit-workers
```

## Configuration

### Worker Configuration

Edit `k8s/configmap.yaml` to customize:

```yaml
data:
  worker.conf: |
    worker_id: "auto"
    pool: "default"
    max_concurrent: 4
    jobs_dir: /var/lib/guestkit/jobs
    results_dir: /var/lib/guestkit/results
    job_timeout_seconds: 7200
    log_level: info
```

### Resource Limits

Edit `k8s/daemonset.yaml` to adjust resources:

```yaml
resources:
  requests:
    cpu: 500m
    memory: 512Mi
  limits:
    cpu: 2
    memory: 2Gi
```

## Submitting Jobs

### 1. Create Job File

```bash
cat > inspect-job.json <<'EOF'
{
  "version": "1.0",
  "job_id": "inspect-001",
  "operation": "guestkit.inspect",
  "payload": {
    "type": "guestkit.inspect.v1",
    "data": {
      "image": {
        "path": "/data/vm.qcow2",
        "format": "qcow2"
      },
      "options": {
        "include_packages": true,
        "include_services": true,
        "include_security": true
      }
    }
  }
}
EOF
```

### 2. Copy to Worker Pod

```bash
# Get pod name
POD=$(sudo kubectl get pods -n guestkit-workers -l app=guestkit-worker -o name | head -1)

# Copy job file
sudo kubectl cp inspect-job.json guestkit-workers/${POD#pod/}:/var/lib/guestkit/jobs/

# Check result
sudo kubectl exec -n guestkit-workers ${POD#pod/} -- ls -la /var/lib/guestkit/results/
```

### 3. View Results

```bash
# Get result file
sudo kubectl exec -n guestkit-workers ${POD#pod/} -- \
  cat /var/lib/guestkit/results/inspect-001-result.json
```

## Monitoring

### View Logs

```bash
# All workers
sudo kubectl logs -n guestkit-workers -l app=guestkit-worker -f

# Specific worker
sudo kubectl logs -n guestkit-workers guestkit-worker-xxxxx -f
```

### Check Status

```bash
# Pod status
sudo kubectl get pods -n guestkit-workers -o wide

# DaemonSet status
sudo kubectl describe daemonset -n guestkit-workers guestkit-worker

# Events
sudo kubectl get events -n guestkit-workers --sort-by='.lastTimestamp'
```

## Troubleshooting

### Pods Not Starting

```bash
# Check events
sudo kubectl describe pod -n guestkit-workers guestkit-worker-xxxxx

# Check logs
sudo kubectl logs -n guestkit-workers guestkit-worker-xxxxx

# Check node labels
sudo kubectl get nodes --show-labels | grep guestkit
```

### Image Pull Errors

```bash
# For k3d, import image again
sudo k3d image import guestkit-worker:latest -c hyper2kvm-test

# Verify image in cluster
sudo docker exec k3d-hyper2kvm-test-server-0 crictl images | grep guestkit
```

### Jobs Not Processing

```bash
# Check worker is running
sudo kubectl exec -n guestkit-workers guestkit-worker-xxxxx -- \
  pgrep -f guestkit-worker

# Check job directory
sudo kubectl exec -n guestkit-workers guestkit-worker-xxxxx -- \
  ls -la /var/lib/guestkit/jobs/
```

## Scaling

### Add More Workers

```bash
# Label additional nodes
sudo kubectl label nodes <node-name> guestkit.io/worker-enabled=true
```

### Remove Workers

```bash
# Remove label from nodes
sudo kubectl label nodes <node-name> guestkit.io/worker-enabled-
```

## Cleanup

```bash
# Delete all resources
sudo kubectl delete -k k8s/

# Or delete individually
sudo kubectl delete namespace guestkit-workers
```

## Architecture

```
┌─────────────────────────────────────────┐
│         Kubernetes Cluster              │
│                                         │
│  ┌───────────────────────────────────┐  │
│  │   Namespace: guestkit-workers     │  │
│  │                                   │  │
│  │  ┌─────────────────────────────┐  │  │
│  │  │  DaemonSet: guestkit-worker │  │  │
│  │  │                             │  │  │
│  │  │  ┌──────────┐ ┌──────────┐  │  │  │
│  │  │  │ Worker 1 │ │ Worker 2 │  │  │  │
│  │  │  │ (Agent 0)│ │ (Agent 1)│  │  │  │
│  │  │  └──────────┘ └──────────┘  │  │  │
│  │  │       ↓            ↓        │  │  │
│  │  │  ┌────────────────────┐    │  │  │
│  │  │  │  /var/lib/guestkit │    │  │  │
│  │  │  │  ├─ jobs/          │    │  │  │
│  │  │  │  └─ results/       │    │  │  │
│  │  │  └────────────────────┘    │  │  │
│  │  └─────────────────────────────┘  │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

## Security Considerations

- Workers run as privileged containers (required for NBD/loop devices)
- ServiceAccount has minimal RBAC permissions
- Jobs are isolated per worker
- Results are stored in emptyDir volumes

## Production Recommendations

1. **Use PersistentVolumes** for job/result storage
2. **Configure resource limits** based on VM sizes
3. **Set up monitoring** with Prometheus/Grafana
4. **Enable network policies** for isolation
5. **Use secrets** for sensitive configuration
6. **Implement job queue** (Redis/Kafka) instead of file transport
7. **Add health checks** for better reliability

## See Also

- [deploy/README.md](../deploy/README.md) - Local quickstart and deployment
- [docs/architecture/overview.md](../docs/architecture/overview.md) - Full system overview
