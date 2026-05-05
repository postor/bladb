# ROS2 Bridge Design

This example is meant to show how Bladb can expose a ROS2-style publish and subscribe workflow to frontend teams without giving browsers arbitrary broker access.

## Primary modules

- `ros2`
  - topic publish bridge
  - topic subscription snapshots
  - tenant and robot namespace enforcement
- `redis`
  - optional live counters and room presence in future iterations
- `nats + jetstream`
  - internal fan-out
  - replay and worker hooks for robotics telemetry pipelines

## Frontend paths

The example keeps two user-facing pages:

1. `publish`
   - send a ROS2 command to an allowed topic
   - payload still looks like native message content
2. `subscribe`
   - poll the latest allowed messages for one topic
   - read recent message history owned by the current tenant

The browser should never subscribe to wildcard topics directly. It only reads the topic history that the backend module has already filtered and approved.

## Why this split matters

ROS2 is event-driven, but browsers are not trusted peers on the robotics bus.

The bridge module therefore owns:

- topic allowlisting
- tenant namespace binding
- actor stamping
- replay-safe message snapshots for UI subscribers

That gives frontend teams a native-feeling publish and subscribe demo while keeping the real broker boundary in Rust.

## Docker backend

The example stack now also includes a dedicated Docker-managed ROS backend service.

- service name: `ros2-backend`
- health endpoint: `GET /health`
- publish endpoint: `POST /messages`
- subscribe snapshot endpoints:
  - `GET /messages/:topicName`
  - `GET /messages/:topicName/latest`

In the current repository state, the Docker service is live and documented, while the gateway-side proxy path is still intentionally reserved for the next wiring step.
