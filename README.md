# Bladb

Bladb is a mono repo for a database gateway + frontend SDK platform.

The goal is simple:

- keep the call surface close to native SQL / Mongo / Redis usage
- let frontend developers build high-concurrency, distributed, and realtime features
- enforce identity, tenant isolation, and filter-based safety in the backend
- make cross-database access feel consistent without forcing a brand-new query language
- support stream and queue backends such as MQTT, Kafka, and MQ without turning them into fake SQL

## Design goals

1. Native-looking calls

Frontend code should still feel familiar:

```ts
await db.sql`select * from orders where uid = ${UID} and status = ${status}`;
await db.mongo("devices").find({ ownerUid: UID, status: "online" });
await db.redis.incrby(key`${UID}_wallet`, 10);
```

2. Reserved identity keys

Bladb reserves a small set of context values that line up with JWT claims and backend policy config:

- `UID` -> `jwt.claims.uid`
- `TENANT_ID` -> `jwt.claims.tenantId`
- `ROLES` -> `jwt.claims.roles`
- `PERMISSION_VERSION` -> `jwt.claims.permissionVersion`

The frontend can use these predefined values directly. The backend resolves them from JWT instead of trusting user input.

3. Policy-first execution

Requests look native at the SDK layer, but the server still owns execution:

- authenticate JWT
- resolve reserved values such as `UID`
- match compiled policy rules
- inject tenant and identity filters
- validate command / collection / table access
- execute against the selected module
- audit the request

4. Performance first

Bladb should avoid heavy runtime analysis on the hot path.

The preferred model is:

- startup: load YAML / JSON / code policies
- startup: compile policies into fast match rules
- request time: match + bind + validate + execute

Complex dynamic permission logic is allowed, but the default path should stay cheap.

## Monorepo layout

```txt
bladb/
  apps/
    docs/
    examples/
      flash-sale/
      iot-realtime/
  packages/
    client/
    react/
  crates/
    core/
    gateway/
    module-*/
  Cargo.toml
  package.json
  pnpm-workspace.yaml
```

## Planned responsibilities

### Rust crates

- `core`: shared protocol, errors, request context, reserved-value model
- `gateway`: auth, policy match, module dispatch, audit
- `module-*`: MySQL, PostgreSQL, MongoDB, Redis, Memcache adapters
- `auth`: JWT parsing, key rotation, claim mapping
- `policy`: YAML / JSON / code policy compiler
- `stream`: event envelope, publish / subscribe protocol, delivery metadata
- `worker`: background execution, retry, dead-letter, idempotency, scheduling

### JS / TS packages

- `@bladb/client`: native-looking client API for SQL / Mongo / Redis
- `@bladb/react`: hooks for query, mutation, and live updates
- `@bladb/vue`: planned composables package

### Apps

- `apps/docs`: product docs, policy cookbook, SDK guides
- `apps/examples/flash-sale`: high-concurrency stock + order demo
- `apps/examples/iot-realtime`: massive device telemetry + realtime control demo

## Reserved values

Reserved values keep the frontend API simple while making backend policy configuration predictable.

```ts
import { UID, TENANT_ID, key } from "@bladb/client";

await db.sql`select * from orders where uid = ${UID}`;
await db.mongo("devices").find({ ownerUid: UID, tenantId: TENANT_ID });
await db.redis.get(key`${UID}_wallet`);
```

Why this matters:

- frontend code stays close to native usage
- backend filters can use the same names
- JWT claim mapping stays obvious
- accidental cross-user access becomes much harder

## Module categories

Bladb should not treat every backend as a "database".

The platform model is split into four categories:

- `data modules`: `mysql`, `pg`, `mongo`, `redis`, `memcache`
- `stream modules`: `mqtt`, `kafka`, `redis-streams`, `nats`
- `queue modules`: `rabbitmq`, `rocketmq`, `sqs`, delayed-job backends
- `worker runtime`: background consumers, cron jobs, compensations, fan-out tasks

This split keeps the API honest:

- `sql / mongo / redis` stay native-looking for reads and direct commands
- `mqtt / kafka / mq` stay native-looking for publish / consume semantics
- `worker` handles cross-module workflows instead of forcing the frontend to orchestrate them

## Policy examples

Bladb should support YAML, JSON, or code-based policy definitions.

Example YAML:

```yaml
auth:
  jwt:
    claims:
      UID: uid
      TENANT_ID: tenantId
      ROLES: roles

policies:
  - name: orders.select-mine
    match:
      engine: sql
      operation: select
      tables: [orders]
    enforce:
      where:
        uid: "{UID}"
        tenant_id: "{TENANT_ID}"

  - name: wallet.read-mine
    match:
      engine: redis
      command: get
    enforce:
      key:
        exact: "{UID}_wallet"
```

Example code config:

```ts
definePolicy({
  name: "devices.find-mine",
  match: { engine: "mongo", collection: "devices", action: "find" },
  enforce: {
    query: {
      ownerUid: "{UID}",
      tenantId: "{TENANT_ID}"
    }
  }
});
```

## Cross-module changes

When one module causes another module to change, Bladb should not rely on hidden module-to-module side effects.

Preferred execution modes:

1. `sync`
   Use for short, request-bound work that must complete before the user gets a response.

2. `event`
   Use when a change should be announced to other systems or consumers.

3. `worker`
   Use for retries, compensations, indexing, analytics, notifications, timeout jobs, and other eventual-consistency work.

Recommended flow:

```txt
frontend request
-> gateway
-> primary module command
-> event envelope
-> stream / queue backend
-> worker consumes event
-> worker updates other modules
```

This avoids tight coupling between modules and keeps retries, auditing, and permissions explicit.

## Worker model

Workers are a first-class runtime in Bladb.

Each worker should define:

- `name`
- `trigger`
- `source module`
- `input schema`
- `identity mode`
- `retry policy`
- `timeout`
- `idempotency key`
- `dead-letter policy`

Example worker shape:

```yaml
workers:
  - name: order.analytics-sync
    trigger:
      type: event
      topic: order.created
    idempotency:
      keyFrom: event.eventId
    retry:
      maxAttempts: 5
      backoff: exponential
    timeoutMs: 15000
```

### Identity propagation

Every event and job should carry execution context forward:

```json
{
  "eventId": "evt_01",
  "type": "order.created",
  "source": "sql.orders",
  "traceId": "trace_01",
  "actor": {
    "kind": "user",
    "uid": "u_1001",
    "tenantId": "tenant_a",
    "roles": ["buyer"]
  },
  "payload": {
    "orderId": "ord_01",
    "sku": "camera-pro"
  }
}
```

That lets downstream workers keep policy checks and audit logs consistent even when the original request has already returned.

## Messaging backends

Bladb should treat messaging systems as native modules too.

Recommended fits:

- `mqtt`
  Best for device ingress, low-overhead publish / subscribe, retained messages, QoS, and last-will patterns.
- `kafka`
  Best for high-throughput event streams, replay, analytics, audit logs, and decoupled business pipelines.
- `rabbitmq` / `rocketmq` / similar MQ
  Best for task queues, delayed delivery, retries, dead-letter handling, and business workflows.

These backends should map to stream or queue APIs, not be forced into the same surface as SQL queries.

## Example projects

### Flash sale

Focus:

- stock deduction
- wallet balance
- user-scoped order lookup
- realtime inventory updates
- order event fan-out
- delayed payment timeout handling

Core data split:

- `redis`: stock counters, wallet counters, hot-path rate limits
- `mysql` / `postgres`: orders, users, payment records
- `mongo`: item detail, event snapshots, operational dashboards
- `kafka` or `redis-streams`: order and stock events
- `mq`: delayed timeout jobs and notification jobs

### IoT realtime

Focus:

- user-scoped device list
- tenant-scoped telemetry streams
- realtime command dispatch
- online / offline counters
- device ingress from MQTT
- background aggregation and alert workers

Core data split:

- `mongo`: device metadata, telemetry windows
- `redis`: live counters, pub/sub channels, recent event cache
- `sql`: tenants, billing, audit records
- `mqtt`: device uplink / downlink topics
- `kafka`: telemetry fan-out, analytics, alert pipelines

## Current workspace contents

This first pass includes:

- root workspace files
- initial `README`
- a small `@bladb/client` package
- a small `@bladb/react` package
- example app skeletons for flash sale and IoT realtime
- sample policy YAML files that demonstrate `UID` / `TENANT_ID` usage
- scenario architecture notes and worker design drafts for both example apps

## Next milestones

1. Build the Rust protocol and gateway crates.
2. Define the policy compiler IR.
3. Define event, stream, queue, and worker protocol types.
4. Add a docs app with architecture and policy guides.
5. Implement a real backend for the example apps.
6. Add Vue bindings and more modules.
