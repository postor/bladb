# Example Apps

This folder contains scenario-driven demos for Bladb.

- `flash-sale`: inventory, wallet, and user-scoped order flows
- `iot-realtime`: device list, live telemetry, and command dispatch
- `ros2-bridge`: tenant-scoped ROS2 publish and subscribe bridge pages

These apps intentionally use `UID` and `TENANT_ID` directly in frontend code to demonstrate how native-looking calls can still map to backend policies.

Each example should also demonstrate:

- which path is synchronous
- which changes are emitted as events
- which work is moved into background workers
- which module type is being used: data, stream, or queue
