# Bladb

Bladb is a mono repo for a database gateway + frontend SDK platform.

## Config entrypoints

For the unified gateway configuration, start here:

- Default config: [bladb.yml](/D:/study/bladb/bladb.yml)
- Config spec: [apps/docs/bladb-config-spec.md](/D:/study/bladb/apps/docs/bladb-config-spec.md)
- User module guide: [apps/docs/db-user-module.md](/D:/study/bladb/apps/docs/db-user-module.md)

The official user module contract also lives in that config spec under `modules.official.users`. This is the long-term server contract behind the public `db.user` API.

The goal is simple:

- keep the call surface close to native SQL / Mongo / Redis usage
- let frontend developers build high-concurrency, distributed, and realtime features
- enforce identity, tenant isolation, and filter-based safety in the backend
- make cross-database access feel consistent without forcing a brand-new query language
- support stream and queue backends such as MQTT, NATS, Kafka, and MQ without turning them into fake SQL

## Design goals

1. Native-looking calls

Frontend code should still feel familiar:

```ts
await db.sql`select * from orders where uid = ${UID} and status = ${status}`;
await db.mongo("devices").find({ ownerUid: UID, status: "online" });
await db.redis.incrby(key`${UID}_wallet`, 10);
await db
  .withMeta({ resource: "device.command", params: { deviceId } })
  .mqtt.publish(template`tenant/${TENANT_ID}/devices/${deviceId}/commands`, { action: "reboot" });

const session = await db.user.login({
  app: "flash-sale",
  email: "buyer@flash-sale.demo",
  password: "demo123"
});

await db.app("flash-sale").post("queue", { sku: "camera-pro", quantity: 1 });
await db.app("flash-sale").get("summary");
await db.app("iot-realtime").get("commands");
await db.app("iot-realtime").post("commands", { deviceId: "device-001", action: "reboot" });
await db.app("iot-realtime").stream("commands/device-001/stream", {
  onMessage(event) {
    console.log(event.topic, event.action);
  }
});
await db.app("ros2-bridge").post("messages", {
  robotId: "robot-001",
  topicName: "cmd_vel",
  messageType: "geometry_msgs/msg/Twist",
  payload: { linear: { x: 0.4 }, angular: { z: 0.15 } }
});
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

5. NATS + K8s by default

The default internal service baseline is:

- NATS request/reply for gateway to module RPC
- JetStream for durable events, retries, dead letters, and worker consumers
- Kubernetes deployments with rolling updates and autoscaling metadata driven from topology and worker manifests

That gives Bladb a path to split hot modules and workers independently without changing the frontend API.

## Monorepo layout

```txt
bladb/
  apps/
    docs/
    examples/
      flash-sale/
      iot-realtime/
      ros2-bridge/
  packages/
    client/
    react/
  crates/
    bladb-core/
    bladb-gateway/
    bladb-module-runtime/
    bladb-worker-runtime/
    module-*/
  Cargo.toml
  package.json
  pnpm-workspace.yaml
```

## Planned responsibilities

### Rust crates

- `core`: shared protocol, errors, request context, reserved-value model, cluster topology model
- `gateway`: auth, policy match, module dispatch, audit
- `module-runtime`: generic Rust runtime that boots one logical module cluster from topology config and dispatches into backend adapters
- `worker-runtime`: generic Rust runtime that boots one worker definition from worker manifests and dispatches steps into backend executors
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
- `apps/examples/flash-sale`: anonymous direct-entry stock + queue-order demo
- `apps/examples/blog`: public-read plus authenticated editor demo built on `mongo + user`
- `apps/examples/iot-realtime`: anonymous direct-entry device telemetry + command demo
- `apps/examples/ros2-bridge`: anonymous direct-entry ROS2-style publish and subscribe bridge demo
- `apps/examples/user-module-demo`: dedicated `db.user` login/register/me/logout verification workbench
- `apps/examples/examples-portal`: suite home with runtime URLs, credentials, and the recommended example tour

The example apps now behave as a connected suite:

- the new `examples-portal` is the recommended starting point
- each page includes shared cross-example navigation
- navigation URLs follow the resolved local stack ports instead of assuming fixed defaults
- the recommended progression is anonymous business flows -> mixed public/auth demo -> dedicated auth workbench

## Reserved values

Reserved values keep the frontend API simple while making backend policy configuration predictable.

```ts
import { UID, TENANT_ID, key, template } from "@bladb/client";

await db.sql`select * from orders where uid = ${UID}`;
await db.mongo("devices").find({ ownerUid: UID, tenantId: TENANT_ID });
await db.redis.get(key`${UID}_wallet`);
await db.mqtt.publish(template`tenant/${TENANT_ID}/devices/${deviceId}/commands`, payload);
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
- `stream modules`: `mqtt`, `nats`, `kafka`, `redis-streams`
- `queue modules`: `rabbitmq`, `rocketmq`, `sqs`, NATS subject-based job lanes, delayed-job backends
- `worker runtime`: background consumers, cron jobs, compensations, fan-out tasks

This split keeps the API honest:

- `sql / mongo / redis` stay native-looking for reads and direct commands
- `mqtt / kafka / mq` stay native-looking for publish / consume semantics
- `worker` handles cross-module workflows instead of forcing the frontend to orchestrate them

## Distributed topology

The Rust side should reserve distributed behavior without leaking cluster complexity into the frontend API.

Recommended boundaries:

- `gateway cluster`
  Stateless request routers. They authenticate, match policy, derive route keys, and forward work.
- `module clusters`
  Logical backends such as `flashsale.stock-redis` or `iot.devices-mongo`. A logical module may have many physical nodes.
- `worker clusters`
  Dedicated consumers for event, retry, timeout, projection, and compensation work.
- `control plane`
  Service discovery, topology config, health, and rollout metadata. This can start as YAML and later move to registry-backed discovery.

Important rule:

- frontend calls never choose node, shard, broker partition, or replica directly
- routing comes from backend config plus request context such as `UID`, `TENANT_ID`, `meta.params`, and event payload fields

Recommended rollout path:

1. Keep `bladb-gateway` stateless so it can scale horizontally first.
2. Introduce logical module names and topology manifests before introducing real cluster membership.
3. Route by stable business keys such as `tenantId`, `deviceId`, `sku`, or `orderId`.
4. Split hot modules first, not every module at once.

Unified startup config spec:

- [apps/docs/bladb-config-spec.md](/D:/study/bladb/apps/docs/bladb-config-spec.md)

### Logical module before physical cluster

Do not bind policies to one concrete host.

Prefer:

- policy matches `flashsale.stock.read`
- gateway resolves that policy to logical cluster `flashsale.stock-redis`
- topology decides which physical node handles the request

That preserves one stable policy surface even if Redis later becomes a 16-shard cluster or moves behind a service registry.

### Route key design

Use keys that preserve tenant isolation and business locality:

- flash sale stock: `tenantId + sku`
- orders: `tenantId + orderId`
- device commands: `tenantId + deviceId`
- telemetry streams: `tenantId + deviceId`

Avoid random routing for workflows that need read-your-write behavior or per-entity ordering.

### Event and worker partitioning

Events should carry partition hints even when the first version runs on a single broker or a single JetStream stream.

Recommended event metadata:

- `partitionKey`
- `orderingKey`
- `traceId`
- actor identity and tenant context

That gives JetStream, Kafka, MQ, or Redis Streams workers a stable key for:

- ordered per-device or per-order processing
- idempotent retry
- module-local fan-out
- easier future rebalancing

### Example topology manifests

The repo now includes example topology manifests:

- [apps/examples/flash-sale/topology/flash-sale.topology.yaml](/D:/study/bladb/apps/examples/flash-sale/topology/flash-sale.topology.yaml)
- [apps/examples/iot-realtime/topology/iot-realtime.topology.yaml](/D:/study/bladb/apps/examples/iot-realtime/topology/iot-realtime.topology.yaml)
- [apps/examples/ros2-bridge/topology/ros2-bridge.topology.yaml](/D:/study/bladb/apps/examples/ros2-bridge/topology/ros2-bridge.topology.yaml)

These describe logical module clusters, policy ownership, discovery mode, routing strategy, consistency, and failover without changing the frontend API.

The `/topology` snapshot exposed by the local gateway now includes:

- discovery and service identity
- transport metadata such as `protocol`, `subject`, `queueGroup`, `stream`, `durable`
- deployment metadata such as `replicas`, `rolling`, and `autoscale`

You can dry-run routing locally:

```txt
cargo run -p bladb-gateway -- route apps/examples/flash-sale/policies/flash-sale.policy.yaml apps/examples/flash-sale/topology/flash-sale.topology.yaml apps/examples/flash-sale/gateway/request.orders-read.json apps/examples/flash-sale/gateway/auth.buyer.json

cargo run -p bladb-gateway -- route apps/examples/iot-realtime/policies/iot-realtime.policy.yaml apps/examples/iot-realtime/topology/iot-realtime.topology.yaml apps/examples/iot-realtime/gateway/request.reboot.json apps/examples/iot-realtime/gateway/auth.operator.json
```

The output includes:

- matched policy
- logical cluster name
- target service name
- derived route key
- shard hint
- prepared request body

The dev gateway also exposes HTTP inspection endpoints:

- `GET /health`
- `GET /topology`
- `POST /route`
- `POST /execute`

## Kubernetes baseline

The repo now includes first-pass Kubernetes manifests in [deploy/k8s](/D:/study/bladb/deploy/k8s/README.md).

Current production-shaped baseline:

- `bladb-gateway` is deployable and horizontally scalable today
- `bladb-module-runtime` and `bladb-worker-runtime` now have manifest-driven bootstrap code, typed transport shells, and unrun tests for loop/ack/retry/DLQ behavior
- `NATS + JetStream` is the internal bus baseline
- topology and worker manifests already reserve transport and deployment metadata for future split module and worker binaries

Reference manifests are included for the next two generic runtimes:

- `bladb-module-runtime`
- `bladb-worker-runtime`

Example runtime configs are also checked in so the same topology and worker manifests can drive local dry-runs, future smoke tests, and Kubernetes bootstraps:

- [flash-sale runtime configs](/D:/study/bladb/apps/examples/flash-sale/runtime/flashsale.orders.runtime.yaml)
- [iot runtime configs](/D:/study/bladb/apps/examples/iot-realtime/runtime/iot.commands.runtime.yaml)
- [ros2 runtime configs](/D:/study/bladb/apps/examples/ros2-bridge/runtime/ros2.bridge.runtime.yaml)

## Internal Bus Contract

Bladb now also reserves a shared internal runtime bus model in [crates/bladb-core/src/bus.rs](/D:/study/bladb/crates/bladb-core/src/bus.rs).

This gives the split services one stable language for:

- gateway to module RPC
- JetStream to worker dispatch
- execution reports and future retry / DLQ metadata

The important rule is that module runtimes receive the prepared request body, not the original browser-facing template payload. That keeps reserved-value resolution and routing decisions on the trusted side.

The current split-runtime boundary is now:

- `ModuleRuntimeService` validates internal RPC envelopes, then dispatches into backend adapters
- `ModuleRuntimeRunner` executes one RPC or a bounded batch, and `ModuleTransportServer` adds idle backoff and loop tuning for future NATS request/reply bindings
- `WorkerRuntimeService` validates worker subscription metadata, then executes worker steps
- `WorkerRuntimeRunner` executes one job or a bounded batch, and `WorkerTransportConsumer` centralizes ack / retry / dead-letter / report behavior for future JetStream or Kafka bindings

Runtime configs now also reserve loop tuning knobs so hot module and worker binaries can trade CPU wakeups for lower latency without changing application code:

- `transportLoop.maxBatch`
- `transportLoop.idleSleepMs`
- `workerLoop.maxBatch`
- `workerLoop.idleSleepMs`

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

  - name: device-command.publish
    match:
      engine: mqtt
      action: publish
    enforce:
      topic:
        template: "tenant/{TENANT_ID}/devices/{args.deviceId}/commands"
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

## Client request protocol

The SDK should send a richer request contract than the original prototype.

Recommended request shape:

```json
{
  "kind": "query",
  "engine": "sql",
  "action": "query",
  "meta": {
    "resource": "orders.readMine",
    "policy": "flashsale.orders.read-mine",
    "traceId": "trace_01"
  },
  "statement": "select * from orders where uid = ?",
  "values": [
    { "$ctx": "uid", "token": "UID" }
  ]
}
```

This keeps the API native-looking while still giving the gateway stable metadata for policy matching, tracing, and audit.

## Extraction CLI

The JS client package also includes a project scanner CLI for backend setup work.

It walks a frontend codebase, extracts Bladb calls such as:

- `db.sql`
- `db.mongo(...).find(...)`
- `db.redis.incrby(...)`
- `db.withMeta(...).mqtt.publish(...)`
- `db.kafka.produce(...)`
- `db.mq.publishDelayed(...)`

and emits:

- extracted operations with file and line info
- `resource / policy / params` metadata when present
- suggested backend policy stubs

Example:

```txt
node packages/client/src/cli.mjs apps/examples
pnpm extract:examples
```

Optional flags:

```txt
node packages/client/src/cli.mjs apps/examples --output bladb-ops.json
node packages/client/src/cli.mjs apps/examples --no-suggestions
```

The browser SDK also now includes small gateway helpers that the examples use directly:

- `db.user.login/register/me`
- `db.user.logout()` when the `db` instance comes from `createBrowserAppModule(...)`
- `db.app("<app-name>").get/post(...)`
- `createBrowserSessionStore(...)`
- `createBrowserAuthModule(...)`
- `appGet(...)` / `appPost(...)` / `createTypedAppClient(...)`
- `createBrowserAppModule(...)`

`db.auth` still exists as a compatibility alias in the current repo, but `db.user` is now the official module surface. A plain `createClient(...)` instance still exposes transport-oriented `login/register/me`; the browser-managed `db` returned by `createBrowserAppModule(...)` layers session persistence and `logout()` on top of that same user module.

The React package also now includes:

- `useGatewaySession(...)`
- `useUserSession(...)`

The example apps use that split intentionally:

- `packages/client`: gateway protocol, auth calls, app endpoints, browser session storage
- `packages/react`: query/mutation polling plus gateway session lifecycle
- `apps/examples/*/src/bladb.ts`: app-specific typed wrappers such as queue and command-history clients

Example typed app client pattern:

```ts
const flashSaleApi = createTypedAppClient(db.app("flash-sale"), {
  queuePurchase: appPost<{ sku: string; quantity: number }, QueueTicket>("queue"),
  queueHistory: appGet<QueueTicket[]>("queue"),
  queueTicket: appGet<[string], QueueTicket>((ticketId) => `queue/${ticketId}`)
});
```

Example browser app module pattern:

```ts
const flashSaleModule = createBrowserAppModule({
  baseUrl: BLADB_URL,
  appName: "flash-sale",
  tokenKey: "bladb.flash-sale.token",
  sessionKey: "bladb.flash-sale.session",
  routes: {
    queuePurchase: appPost<{ sku: string; quantity: number }, QueueTicket>("queue"),
    queueHistory: appGet<QueueTicket[]>("queue")
  }
});

export const db = flashSaleModule.db;
export const flashSaleUser = flashSaleModule.user;
export const flashSaleAuth = flashSaleModule.auth;
export const flashSaleApi = flashSaleModule.api;
```

That lets React screens and browser logic stay on the official `db.user` surface instead of wiring a low-level transport client and `sessionStore` separately:

```ts
const user = useGatewaySession(db.user);
await db.user.login({
  app: "flash-sale",
  email: "buyer@flash-sale.demo",
  password: "demo123"
});
db.user.logout();
```

For new auth-focused screens, prefer `useUserSession(...)` with the browser-managed module user handle:

```ts
const flashSaleModule = createBrowserAppModule({
  baseUrl: BLADB_URL,
  appName: "flash-sale",
  tokenKey: "bladb.flash-sale.token",
  sessionKey: "bladb.flash-sale.session",
  routes: {}
});

const session = useUserSession(flashSaleModule.user);
await session.login({
  app: "flash-sale",
  email: "buyer@flash-sale.demo",
  password: "demo123"
});
await session.refresh();
session.logout();
```

The dedicated guide at [apps/docs/db-user-module.md](/D:/study/bladb/apps/docs/db-user-module.md) collects:

- concrete `modules.official.users` config recipes
- when to choose `jwt.secret` vs key files
- when to choose MySQL vs MongoDB storage config
- when SMTP config is required
- which user-module fields are fully active today vs still contract-first

## Running The Example Stack

The repository now includes a config-driven Rust gateway that serves the current example apps using reusable module runtimes instead of a demo-only server path.

Start the full local stack:

```txt
pnpm dev:examples
```

This starts:

- the Rust gateway on `127.0.0.1:8787`
- the ros2-backend service on `127.0.0.1:8080`
- the examples-portal app on `127.0.0.1:4172`
- the flash-sale app on `127.0.0.1:4173`
- the blog app on `127.0.0.1:4174`
- the iot-realtime app on `127.0.0.1:4175`
- the ros2-bridge app on `127.0.0.1:4176`
- the user-module-demo app on `127.0.0.1:4177`

If one of those host ports is already occupied, `pnpm dev:examples` and `pnpm dev:examples:local` now auto-pick the next free local ports and print the resolved URLs after startup.

If you want to pin ports for this project, you can set any of:

- `BLADB_GATEWAY_PORT`
- `BLADB_ROS2_BACKEND_PORT`
- `BLADB_EXAMPLES_PORTAL_PORT`
- `BLADB_FLASH_SALE_PORT`
- `BLADB_BLOG_PORT`
- `BLADB_IOT_PORT`
- `BLADB_ROS2_PORT`
- `BLADB_USER_MODULE_DEMO_PORT`

By default the gateway auto-discovers:

- [bladb.yml](/D:/study/bladb/bladb.yml)
- Config spec: [apps/docs/bladb-config-spec.md](/D:/study/bladb/apps/docs/bladb-config-spec.md)

That file sets:

- `mode: standalone` for the local single-binary gateway path
- `runtime.role` for future shared cluster/runtime bootstraps when not running standalone

That file owns:

- runtime policy/topology bindings
- seeded local auth users
- official module contracts such as `modules.official.users`
- local module seed data for flash-sale and iot
- local module seed data for blog
- local module seed data for ros2 publish and subscribe flows

You can point the same binary at another config:

```txt
cargo run -p bladb-gateway -- serve 127.0.0.1:8787 bladb.yml

BLADB_GATEWAY_CONFIG=bladb.yml cargo run -p bladb-gateway -- serve
```

The older gateway-only fixture still exists here:

- [apps/examples/gateway/local-gateway.yaml](/D:/study/bladb/apps/examples/gateway/local-gateway.yaml)

It remains useful as a narrow local gateway config fixture, but the repo-level `bladb.yml` is now the compose-like default entrypoint.

You can still run them separately if needed:

```txt
pnpm dev:gateway
pnpm --dir apps/examples/flash-sale dev --host 127.0.0.1 --port 4173
pnpm --dir apps/examples/blog dev --host 127.0.0.1 --port 4174
pnpm --dir apps/examples/iot-realtime dev --host 127.0.0.1 --port 4175
pnpm --dir apps/examples/ros2-bridge dev --host 127.0.0.1 --port 4176
pnpm --dir apps/examples/user-module-demo dev --host 127.0.0.1 --port 4177
```

Smoke test the already-running stack:

```txt
pnpm smoke:examples
```

Current local URLs:

- examples portal: `http://127.0.0.1:4172`
- flash sale: `http://127.0.0.1:4173`
- blog: `http://127.0.0.1:4174`
- iot realtime: `http://127.0.0.1:4175`
- ros2 bridge: `http://127.0.0.1:4176`
- user module demo: `http://127.0.0.1:4177`
- gateway health: `http://127.0.0.1:8787/health`
- gateway topology: `http://127.0.0.1:8787/topology`

Seed credentials from the local gateway config:

- flash-sale buyer: `buyer@flash-sale.demo` / `demo123`
- blog editor: `editor@blog.demo` / `demo123`
- iot operator: `operator@iot.demo` / `demo123`
- ros2 operator: `operator@ros2.demo` / `demo123`
- user-module-demo member: `member@user.demo` / `demo123`

Gateway endpoints:

- `POST /auth/register`
- `POST /auth/login`
- `GET /auth/me`
- `POST /users/register`
- `POST /users/login`
- `GET /users/me`
- `POST /route`
- `POST /execute`
- `GET /topology`
- `POST /apps/flash-sale/queue`
- `GET /apps/flash-sale/summary`
- `GET /apps/flash-sale/queue`
- `GET /apps/flash-sale/queue/:ticketId`
- `POST /apps/iot-realtime/commands`
- `GET /apps/iot-realtime/commands`
- `GET /apps/iot-realtime/commands/:deviceId/stream`
- `POST /apps/ros2-bridge/messages`
- `GET /apps/ros2-bridge/messages/:topicName`
- `GET /apps/ros2-bridge/messages/:topicName/latest`
- `GET /apps/ros2-bridge/messages/:topicName/stream`

These `/apps/*` endpoints are now module-owned application APIs, not hardcoded branches in the gateway entrypoint. That keeps example-specific workflow HTTP routes on the same extension path future production modules can use.

## Manual Verification

### Browser verification

- `examples-portal`: open `http://127.0.0.1:4172`, confirm the resolved URLs and seed credentials render, then click through into another example.
- `flash-sale`: open `http://127.0.0.1:4173`, confirm the dashboard loads without a login wall, click `Join queue`, and confirm the current ticket and queue history update.
- `blog`: open `http://127.0.0.1:4174`, confirm the public post list loads immediately, then login with `editor@blog.demo` / `demo123`, publish a post, and confirm it appears in both `Published posts` and `My posts`.
- `iot-realtime`: open `http://127.0.0.1:4175`, keep `device-001` selected, click `Reboot device`, and confirm `MQTT stream: subscribed` plus `Last MQTT action`, `Last MQTT topic`, and `Delivered at` all update.
- `ros2-bridge`: open `http://127.0.0.1:4176`, use the `Publish Page` to send `cmd_vel`, switch to `Subscribe Page`, and confirm the live stream card, payload preview, and recent messages update for that topic.
- `user-module-demo`: open the resolved `user-module-demo` URL printed by `pnpm dev:examples`, login with `member@user.demo` / `demo123`, click `Refresh me`, register a new account, then click `Logout` and confirm the session panel returns to `Signed out`.

### CLI verification

- Start the full stack with `pnpm dev:examples`.
- Run `pnpm smoke:examples:local` to verify anonymous example app flows, `db.user` auth and `/users/*` aliases, blog `mongo + user` behavior, and the first live stream event for both IoT and ROS2 realtime endpoints.
- Run `node --experimental-strip-types --test packages/client/test/browser-module.test.ts` to verify the browser module client behavior, including `db.user` and SSE keepalive handling.

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
- `bladb-core` with tested protocol, policy, event, and worker models
- `bladb-gateway` with authorization, request preparation, topology routing, config-driven local serving, and a dry-run CLI
- example app skeletons for flash sale and IoT realtime
- sample policy YAML files that demonstrate `UID` / `TENANT_ID` usage
- scenario architecture notes and worker design drafts for both example apps

## Next milestones

1. Build the Rust protocol and gateway crates.
2. Define the policy compiler IR.
3. Define event, stream, queue, and worker protocol types.
4. Add a docs app with architecture and policy guides.
5. Replace in-memory local module runtimes with concrete MySQL / Postgres / Mongo / Redis / MQTT adapters behind the same gateway config shape.
6. Add Vue bindings and more modules.
