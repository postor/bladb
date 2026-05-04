# Kubernetes Deployment

This directory contains the first production-shaped Kubernetes manifests for Bladb.

## Current baseline

Today the repo ships one deployable Rust service:

- `bladb-gateway`

It already understands:

- policy manifests
- topology manifests
- auth users
- module-owned app APIs
- cluster transport metadata
- deployment metadata

The current example gateway still runs the example module runtimes in-process. That keeps the stack runnable today while the topology contract is being prepared for the next step: separate module services and worker runtimes.

## Files

- [bladb-example-stack.yaml](/D:/study/bladb/deploy/k8s/bladb-example-stack.yaml)
  Example namespace, NATS with JetStream, gateway config, gateway deployment, service, and HPA.
- [reference/bladb-module-runtime.yaml](/D:/study/bladb/deploy/k8s/reference/bladb-module-runtime.yaml)
  Reference shape for a future independently scaled module runtime.
- [reference/bladb-worker-runtime.yaml](/D:/study/bladb/deploy/k8s/reference/bladb-worker-runtime.yaml)
  Reference shape for a future independently scaled worker runtime.

Runtime example configs also live beside the example apps:

- [flash-sale runtime configs](/D:/study/bladb/apps/examples/flash-sale/runtime/flashsale.orders.runtime.yaml)
- [iot-realtime runtime configs](/D:/study/bladb/apps/examples/iot-realtime/runtime/iot.commands.runtime.yaml)

## Apply the example stack

```bash
kubectl apply -f deploy/k8s/bladb-example-stack.yaml
```

The example stack expects these images to exist:

- `ghcr.io/bladb/bladb-gateway:latest`

The internal service bus baseline is:

- NATS for request/reply and lightweight service routing
- JetStream for durable event streams, worker consumers, retries, and dead letters

## Why this shape

- `gateway` is stateless and horizontally scalable
- `NATS` gives a uniform internal transport for module RPC and worker fan-out
- topology manifests already declare `transport` and `deployment`, so the control-plane contract is stable before the codebase is fully split into separate module binaries
- `HPA` and rolling strategy are defined where the request load actually lands first

## Next deployment step

The next production step is to introduce two more generic Rust binaries:

- `bladb-module-runtime`
- `bladb-worker-runtime`

Those binaries are now scaffolded in the workspace and consume the same topology and worker manifests already present in this repo, so the split is additive rather than a rewrite.
