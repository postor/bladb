# Example Apps

This folder contains scenario-driven demos for Bladb.

- `examples-portal`: suite home with resolved URLs, seed credentials, and the recommended tour
- `flash-sale`: inventory, wallet, and user-scoped order flows
- `blog`: public reading plus editor login flows using `mongo + user`
- `iot-realtime`: device list, live telemetry, and command dispatch
- `ros2-bridge`: tenant-scoped ROS2 publish and subscribe bridge pages
- `user-module-demo`: dedicated login, register, me, and logout verification page for `db.user`

These apps intentionally use `UID` and `TENANT_ID` directly in frontend code to demonstrate how native-looking calls can still map to backend policies.

Each example should also demonstrate:

- which path is synchronous
- which changes are emitted as events
- which work is moved into background workers
- which module type is being used: data, stream, or queue

Current entry modes:

- `flash-sale`, `iot-realtime`, and `ros2-bridge` are anonymous direct-entry demos backed by seeded runtime identities.
- `blog` mixes public reads with authenticated editor actions through `db.user`.
- `user-module-demo` remains the dedicated official auth contract workbench.

Primary learning goals:

- `flash-sale`: queue-first purchase UX, app summary aggregation, worker-settled order state, and the anonymous app API shape a frontend would call.
- `blog`: public app-owned reads plus authenticated `db.mongo` writes through `db.user`.
- `iot-realtime`: tenant-scoped device reads, module-owned command publishing, first-event realtime feedback, and the backend-owned MQTT boundary.
- `ros2-bridge`: ROS2-style publish/subscribe UX on top of filtered app APIs and stream routes, plus the trusted boundary between browser intent and bridge-owned topic stamping.
- `user-module-demo`: direct verification of `db.user.login`, `register`, `me`, and `logout`.

Recommended traversal:

1. Start with `examples-portal` to collect the resolved stack URLs, credentials, and the recommended suite order.
2. Move into `flash-sale`, `iot-realtime`, or `ros2-bridge` to understand anonymous direct-entry business flows.
3. Open `blog` next to see how public app reads and authenticated editor writes can live in one page.
4. Finish with `user-module-demo` when you want to inspect the standalone `db.user` contract directly.

The example pages now also expose a shared suite navigation rail so you can jump across demos even when local ports auto-shift, and the portal is the new recommended first page.
The anonymous examples also surface browser-visible route ownership, SDK-shaped usage hints, and backend responsibility notes so they work as teaching pages in addition to runnable demos.
