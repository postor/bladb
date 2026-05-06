# ROS2 Bridge Design

This example shows how Bladb can expose a ROS2-style publish and subscribe workflow to frontend teams without giving browsers arbitrary broker access.

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

## Module-owned app routes

The shared gateway now exposes the ROS2 example through app-owned routes:

- `POST /apps/ros2-bridge/messages`
- `GET /apps/ros2-bridge/messages/:topicName`
- `GET /apps/ros2-bridge/messages/:topicName/latest`
- `GET /apps/ros2-bridge/messages/:topicName/stream`

These are the stable browser-facing routes. When `backend_base_url` is configured, the module can proxy recent/latest/stream reads to the Docker `ros2-backend` service. When it is not configured, the same app contract is served from local module state.

## Frontend paths

The example keeps two user-facing pages:

1. `Publish Page`
   - send a ROS2 command to an allowed topic
   - payload still looks close to native message content
2. `Subscribe Page`
   - read the latest allowed message for one topic
   - watch the filtered live stream for the same topic
   - inspect recent topic history owned by the current tenant

The browser never gets wildcard broker access. It only reaches topic history and live events that the backend module has already filtered and approved.

## Browser verification

1. Start the example stack with `pnpm dev:examples`.
2. Open `http://127.0.0.1:4176`.
3. Confirm the page opens directly in anonymous example mode.
4. On `Publish Page`, keep topic `cmd_vel` and click `ros2 publish`.
5. Switch to `Subscribe Page`.
6. Confirm the page updates all of the following for `cmd_vel`:
   - `Live stream`
   - `Latest robot`
   - payload preview
   - `Recent messages`

## CLI verification

- Run `pnpm smoke:examples:local` against the already-running stack.
- That smoke run covers:
  - anonymous `POST /apps/ros2-bridge/messages`
  - anonymous `GET /apps/ros2-bridge/messages/:topicName/stream` first-event delivery
  - anonymous `GET /apps/ros2-bridge/messages/:topicName/latest`
  - anonymous `GET /apps/ros2-bridge/messages/:topicName`
  - dedicated `/users/*` alias verification on auth-focused examples

## Why this split matters

ROS2 is event-driven, but browsers are not trusted peers on the robotics bus.

The bridge module therefore owns:

- topic allowlisting
- tenant namespace binding
- actor stamping
- replay-safe message snapshots for UI subscribers

That gives frontend teams a native-feeling publish and subscribe demo while keeping the real broker boundary in Rust.

## Docker backend

The example stack also includes a dedicated Docker-managed ROS backend service.

- service name: `ros2-backend`
- health endpoint: `GET /health`
- publish endpoint: `POST /messages`
- subscribe snapshot endpoints:
  - `GET /messages/:topicName`
  - `GET /messages/:topicName/latest`
  - `GET /messages/:topicName/stream`

The browser-facing contract still stays on `/apps/ros2-bridge/*` even when the module proxies to that backend.
