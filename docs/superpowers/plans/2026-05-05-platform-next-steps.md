# Bladb Platform Next Steps Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep Bladb on a credible standalone-first path where the example apps, realtime flows, and official module boundaries are testable and easy for developers to understand.

**Architecture:** One repo-root `bladb.yml` boot path, one shared Rust gateway/runtime surface, example apps that lead with module-owned business APIs, and a gradual expansion from local runtime credibility to future distributed runtimes.

**Tech Stack:** Rust gateway/runtime crates, TypeScript client/react packages, Docker example stack, Vite apps, MQTT and ROS2-style realtime flows.

---

## Status Snapshot

### Completed foundation

- [x] Repo-root `bladb.yml` is the main standalone startup entrypoint.
- [x] `db.user` V1 is implemented and documented as the first official module surface.
- [x] `/users/*` aliases are live and verified by smoke coverage.
- [x] `flash-sale` ships a working login -> queue -> status/result -> history flow.
- [x] `iot-realtime` shows tenant-scoped device state plus MQTT command feedback in the browser.
- [x] `ros2-bridge` ships a working publish/subscribe browser story with tenant-scoped topic reads.
- [x] Root docs and example docs now point developers to browser and CLI verification steps.

### Verified in this workstream

- [x] `node --experimental-strip-types --test packages/client/test/browser-module.test.ts`
- [x] `pnpm build:examples`
- [x] `pnpm dev:examples`
- [x] `pnpm smoke:examples:local`
- [x] Local smoke now verifies the first realtime stream event for both IoT and ROS2 flows
- [x] In-app browser checks for `flash-sale`, `iot-realtime`, and earlier `ros2-bridge`

### Remaining meaningful work

- [ ] Restore a reliable host-native Rust test signal for the gateway/runtime on this Windows machine.
- [ ] Decide whether extra module-level stream subscription logs are worth the noise beyond the current gateway/backend lifecycle logs.
- [ ] Start V2 execution of the official users module contract instead of stopping at declarative config and compatibility transport.

## Track A: Realtime Trust

### Task A1: Instrument realtime lifecycle boundaries

- [x] Add stream open, first-event, close, and error logging at the gateway edge.
- [x] Keep lifecycle fields stable enough to compare ROS2 and IoT flows.
- [x] Avoid noisy per-chunk logging.

### Task A2: Harden browser stream behavior

- [x] Add regression coverage for keepalive/comment frames before the first real SSE event.
- [x] Keep the browser stream API native-looking for app code.
- [ ] Add reconnect behavior only if future tests or browser failures prove a real gap.

### Task A3: Add MQTT app-visible publish/sub path

- [x] Make MQTT a believable browser-visible publish/sub example.
- [x] Keep publish close to native `db.mqtt.publish(...)` semantics where useful.
- [x] Surface subscription through a tenant-scoped app API path.

### Task A4: Browser verification for MQTT

- [x] Document the MQTT browser walkthrough.
- [x] Verify the flow in the browser against the live app.

## Track B: Example Apps As Product Demos

### Task B1: Flash-sale flow completion

- [x] Finish the login -> queue -> status/result -> history buyer path with app-owned APIs.
- [x] Keep the example focused on queue-worker behavior rather than a fake direct stock mutation.

### Task B2: IoT app credibility

- [x] Show live device state and command-side feedback together.
- [x] Make the page demonstrate why MQTT plus workers matter.

### Task B3: ROS2 app polish

- [x] Preserve the publish/sub loop through app-owned routes.
- [x] Clarify tenant scoping, topic naming, and safe frontend behavior in the app and docs.

## Track C: Config And Startup Consolidation

### Task C1: One startup story

- [x] Keep standalone mode first-class in `bladb.yml`.
- [x] Keep `runtime.role` documented for future non-standalone bootstraps.
- [x] Treat example module blocks as real local runtime config, not throwaway demo knobs.

### Task C2: Official module contracts

- [x] Keep `modules.official.users` as the first official module contract.
- [x] Document the contract in both the root README and the config spec.
- [ ] Decide later whether future stream/worker contracts need the same top-level doc treatment.

## Track D: Test And Verification Coverage

### Task D1: Client/runtime tests for realtime behavior

- [x] Cover browser stream framing behavior for keepalive/comment frames.
- [x] Keep ROS2 local stream delivery tests in the runtime/module layer.
- [ ] Expand direct Rust-side coverage further once host-native test execution is trustworthy again.

### Task D2: Smoke and dev scripts

- [x] Keep startup and smoke flows aligned with the unified config story.
- [x] Add smoke coverage for the public `/users/*` aliases.
- [x] Add smoke coverage for IoT and ROS2 stream endpoints, not only history/latest endpoints.

### Task D3: Manual verification checklist

- [x] Root README includes browser and CLI verification notes.
- [x] `iot-realtime` README includes browser verification notes.
- [x] `ros2-bridge` README includes browser and CLI verification notes.

## Track E: Public API Cleanup

### Task E1: Finish `db.user` V1

- [x] Implement and verify the user-module V1 plan.
- [x] Rewrite the plan file as a real status document instead of leaving it design-only.

### Task E2: Keep app APIs leading business workflows

- [x] Keep low-level `sql`, `mongo`, `redis`, and `mqtt` calls available.
- [x] Prefer `/apps/*` module APIs for business commands and session-scoped views.

## Next Execution Batch

1. Restore reliable host-native Rust verification for gateway/runtime work.
2. Revisit stream observability only if the current edge logs are not enough during the next realtime bug.
3. Start V2 work for the official users module runtime: storage, crypto loading, and mailer execution.
