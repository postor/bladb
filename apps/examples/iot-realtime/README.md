# IoT Realtime Design

This example is meant to show how Bladb can combine frontend-native Mongo and Redis usage with stream-native MQTT and worker-driven aggregation.

## Primary modules

- `mongo`
  - device metadata
  - latest telemetry snapshots
  - alert history
- `redis`
  - active device counters
  - fast lookup caches
  - live fan-out channels
- `sql`
  - tenant records
  - billing and audit data
- `mqtt`
  - device uplink telemetry
  - downlink command topics
- `nats + jetstream`
  - telemetry event stream
  - analytics and alert fan-out
  - durable alert and retry consumers

## Synchronous paths

Frontend-facing paths stay simple:

1. read my devices from `mongo`
2. read tenant counters from `redis`
3. send device commands through the module app API

Those paths should remain native-looking and policy bound to `UID` and `TENANT_ID`.

For the frontend app, the preferred path is now the module-owned command API:

```ts
await iotApi.publishCommand({
  deviceId,
  action: "reboot"
});
```

That keeps tenant binding, topic generation, and actor identity on the Rust side. The lower-level policy fixture still exists for dry-run and adapter validation:

```txt
cargo run -p bladb-gateway -- apps/examples/iot-realtime/policies/iot-realtime.policy.yaml apps/examples/iot-realtime/gateway/request.reboot.json apps/examples/iot-realtime/gateway/auth.operator.json
```

The local example stack is served by the shared gateway binary with:

```txt
cargo run -p bladb-gateway -- serve 127.0.0.1:8787 apps/examples/gateway/local-gateway.yaml
```

The example also exposes an app-level command history API through the same gateway:

```txt
POST /apps/iot-realtime/commands
GET /apps/iot-realtime/commands
GET /apps/iot-realtime/commands/:deviceId/stream
```

That route is served by the IoT module itself, not by a special-case gateway branch.

## Browser verification

The current browser flow is meant to make MQTT effects visible without exposing raw broker access in the frontend.

1. Open the IoT app.
2. Login with `operator@iot.demo` / `demo123`.
3. Pick `device-001`.
4. Click `Reboot device`.
5. Confirm the page moves the MQTT stream state to `subscribed` and updates:
   - `Last MQTT action`
   - `Last MQTT topic`
   - `Delivered at`

The frontend reads that event through the app-owned stream route:

```ts
await iotApi.commandEvents("device-001", {
  onMessage(event) {
    console.log(event.topic, event.action);
  }
});
```

## Ingress and background processing

Device telemetry should enter through `mqtt`, not through a fake database write API.

Recommended flow:

1. device publishes telemetry to `mqtt`
2. ingest module validates topic and tenant binding
3. gateway emits `telemetry.received`
4. workers fan out to:
   - update latest telemetry in `mongo`
   - update counters in `redis`
   - append analytics event to `jetstream`
   - generate alerts and notifications

## Worker roles

Recommended workers for this example:

- `telemetry.latest-projector`
  - trigger: `telemetry.received`
  - writes latest snapshot into `mongo`
- `telemetry.counter-updater`
  - trigger: `telemetry.received`
  - refreshes active counters in `redis`
- `telemetry.alert-evaluator`
  - trigger: `telemetry.received`
  - emits `device.alert.raised` when thresholds are crossed
- `device-command-audit`
  - trigger: command publish
  - appends command audit record to `sql` or `jetstream`

## Why this split matters

Realtime ingestion and fan-out belong in stream modules and workers. The frontend app should only care about querying state and sending allowed commands.

The official internal service path for this example is now:

- gateway -> module RPC through `natsService`
- telemetry fan-out through `JetStream`
- worker scaling from stream lag, queue depth, and CPU on Kubernetes
