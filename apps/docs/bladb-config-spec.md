# Bladb Config Spec

This document defines the current unified `bladb.yml` format for Bladb startup.

The goal is one compose-like config file that can support:

- local standalone startup
- future cluster/runtime startup
- shared example and production-oriented bootstrap conventions

## Status

- Current implementation date: `2026-05-05`
- Current runtime wired to this spec: `bladb-gateway`
- Planned next adopters: `bladb-module-runtime`, `bladb-worker-runtime`

## Design rules

1. Prefer one repo-level `bladb.yml` over many role-specific bootstrap files.
2. Keep `standalone` as an explicit runtime mode in config, not in the filename.
3. When not running standalone, prefer config-defined role before environment fallback.
4. Keep standalone config close to real deployment shape so examples do not depend on demo-only server code.

## Discovery

The current gateway startup behavior is:

1. If an explicit config path is passed, use it.
2. Else if `BLADB_GATEWAY_CONFIG` is set, use it.
3. Else search upward from the current working directory for `bladb.yml`.

Current default filename:

```txt
bladb.yml
```

## Top-level shape

```yaml
mode: standalone

runtime:
  role: gateway

gateway:
  runtimes: []
  auth:
    users: []
  modules: {}
```

Top-level keys currently defined:

- `mode`
- `runtime`
- `gateway`

## Mode semantics

### `mode: standalone`

Use the local single-binary gateway path.

Current behavior:

- `gateway` section is required
- gateway builds a local in-process app from configured runtimes, auth users, and module seed state

### `mode` missing or not `standalone`

Treat config as non-standalone runtime bootstrap.

Current behavior:

1. Read `runtime.role` from config.
2. If missing, fallback to `BLADB_RUNTIME_ROLE`.
3. If still missing, startup fails.

This path is intentionally strict so cluster/runtime boot does not accidentally fall back into standalone behavior.

## Runtime section

```yaml
runtime:
  role: gateway
```

Current fields:

- `role`: logical runtime role name

Planned role examples:

- `gateway`
- `module-runtime`
- `worker-runtime`
- future control-plane or scheduler roles if introduced later

## Gateway section

The `gateway` section currently reuses the local gateway config shape.

```yaml
gateway:
  runtimes:
    - name: flash-sale
      policy: apps/examples/flash-sale/policies/flash-sale.policy.yaml
      topology: apps/examples/flash-sale/topology/flash-sale.topology.yaml
      defaultAuth:
        uid: u_2001
        tenantId: tenant_flashsale
        roles:
          - buyer
        permissionVersion: v1
  auth:
    users:
      - app: flash-sale
        uid: u_2001
        tenantId: tenant_flashsale
        email: buyer@flash-sale.demo
        password: demo123
        displayName: Flash Buyer
        roles:
          - buyer
  modules:
    flashSale: {}
    iot: {}
    ros2: {}
```

### `gateway.runtimes`

Defines runtime bindings for the standalone gateway.

Fields:

- `name`: gateway/runtime name visible in topology snapshot
- `policy`: path to policy yaml/json fixture
- `topology`: path to topology yaml/json fixture
- `defaultAuth`: default auth context used by local example execution

### `gateway.auth.users`

Defines seeded local users for standalone auth flows.

Fields:

- `app`
- `uid`
- `tenantId`
- `email`
- `password`
- `displayName`
- `roles`

### `gateway.modules`

Defines local standalone module seed/config blocks.

Current built-in blocks:

- `flashSale`
- `iot`
- `ros2`

These are runtime-specific local module configs, not the long-term full distributed deployment model.

## Relative path rules

For config loaded from `bladb.yml`:

- relative `policy` paths resolve from the directory containing `bladb.yml`
- relative `topology` paths resolve from the directory containing `bladb.yml`

This keeps the config relocatable and compose-like.

## Current example file

The repo-level example and default startup file is:

- [bladb.yml](/D:/study/bladb/bladb.yml)

The older narrow gateway-only fixture still exists here:

- [apps/examples/gateway/local-gateway.yaml](/D:/study/bladb/apps/examples/gateway/local-gateway.yaml)

Use that older file only when a very small gateway-only fixture is useful for tests or focused debugging.

## Error behavior

Current startup errors:

- missing discovered config file
- malformed yaml/json
- `mode: standalone` without a `gateway` section
- non-standalone config without `runtime.role` and without `BLADB_RUNTIME_ROLE`

## Recommended evolution

Next planned extensions should keep this file as the root bootstrap surface and add sections instead of inventing parallel files.

Recommended future sections:

```yaml
moduleRuntime: {}
workerRuntime: {}
cluster: {}
nats: {}
observability: {}
```

But they should only be added when the corresponding runtime is actually wired.

## Non-goals

This spec does not yet define:

- full cluster membership schema
- module-runtime unified boot schema
- worker-runtime unified boot schema
- k8s deployment rendering schema
- secrets management schema

Those should extend this document instead of replacing it.
