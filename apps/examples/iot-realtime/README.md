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
- `kafka`
  - telemetry event stream
  - analytics and alert fan-out

## Synchronous paths

Frontend-facing paths stay simple:

1. read my devices from `mongo`
2. read tenant counters from `redis`
3. publish device commands to `mqtt` or a command stream

Those paths should remain native-looking and policy bound to `UID` and `TENANT_ID`.

## Ingress and background processing

Device telemetry should enter through `mqtt`, not through a fake database write API.

Recommended flow:

1. device publishes telemetry to `mqtt`
2. ingest module validates topic and tenant binding
3. gateway emits `telemetry.received`
4. workers fan out to:
   - update latest telemetry in `mongo`
   - update counters in `redis`
   - append analytics event to `kafka`
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
  - appends command audit record to `sql` or `kafka`

## Why this split matters

Realtime ingestion and fan-out belong in stream modules and workers. The frontend app should only care about querying state and sending allowed commands.
