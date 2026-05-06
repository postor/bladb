# Anonymous Example Suite Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align the whole anonymous example suite around the official `db.user` session model so `flash-sale`, `iot-realtime`, and `ros2-bridge` all work with cookie-backed identity, `db.user.me()` restoration, renewable leases, example smoke coverage, and browser-visible verification.

**Architecture:** Keep the gateway as the single authority for session resolution. Anonymous app requests should resolve to an app-scoped session through the official user module, return a renewable cookie, and pass that resolved identity into module-owned `/apps/*` APIs. `flash-sale` is the reference path; `iot-realtime` and `ros2-bridge` should stop relying on static guest fallbacks and instead consume the same resolved session contract for reads, writes, and live streams.

**Tech Stack:** Rust gateway/runtime code, Node smoke scripts, Vite example apps, browser verification on the local example stack, git on `main`.

---

## File Map

- Modify: `scripts/smoke-examples-local.mjs`
- Modify: `crates/bladb-gateway/src/local/iot.rs`
- Modify: `crates/bladb-gateway/src/local/ros2.rs`
- Modify: `crates/bladb-gateway/src/local/app.rs` if any anonymous session plumbing gap remains
- Modify: `apps/examples/iot-realtime/README.md`
- Modify: `apps/examples/ros2-bridge/README.md`
- Modify: `README.md` if the suite-level verification instructions change
- Verify: running example stack resolved from `.tmp/example-stack-state.json`

## Phase 1: Smoke Coverage And Runtime Freshness

- [ ] Confirm the current `docker-dev` or local example stack is running code that includes anonymous session cookies.
- [ ] Rebuild or restart the example stack if the running gateway is stale relative to `main`.
- [ ] Keep `scripts/smoke-examples-local.mjs` as the suite-level contract for:
  - examples portal availability
  - anonymous UI entry without login walls
  - official `db.user` auth and `/users/*` aliases
  - `flash-sale` cookie-backed anonymous identity + `me` restoration
  - `iot-realtime` anonymous flow
  - `ros2-bridge` anonymous flow
  - `blog` public + `mongo + user` flow
- [ ] Re-run `node scripts/smoke-examples-local.mjs` until the suite either passes or points to a real code gap.

## Phase 2: IoT Anonymous Session Alignment

- [ ] Replace `guest_identity()` fallbacks in `crates/bladb-gateway/src/local/iot.rs` with a resolved app session requirement that matches the `flash-sale` model.
- [ ] Ensure anonymous requests still work by having the gateway mint an app-scoped session before the handler runs, rather than by fabricating identity inside the module.
- [ ] Apply that rule consistently to:
  - command history reads
  - device list and telemetry reads
  - command publish
  - SSE subscriptions
- [ ] Add or update Rust tests so two different anonymous sessions cannot read or subscribe to each other's device-scoped state by accident.

## Phase 3: ROS2 Anonymous Session Alignment

- [ ] Replace `guest_identity()` fallbacks in `crates/bladb-gateway/src/local/ros2.rs` with the same resolved app session contract.
- [ ] Apply that rule consistently to:
  - recent message reads
  - latest message reads
  - publish
  - stream subscription
  - proxy mode passthrough to the backend bridge
- [ ] Add or update Rust tests so anonymous tenant and uid data come from the resolved session and stay stable across request and stream paths.

## Phase 4: Example Docs And Browser Proof

- [ ] Update example READMEs so `iot-realtime` and `ros2-bridge` explicitly describe:
  - anonymous cookie-backed identity
  - renewable session lease on reopen
  - `db.user.me()` restoration
  - module-owned `/apps/*` API usage
- [ ] Re-run browser-visible checks against the resolved example URLs and confirm:
  - anonymous entry works without login UI
  - actions still mutate live state
  - reload keeps the same anonymous identity until logout or expiry

## Phase 5: Commit Strategy

- [ ] Commit smoke-script-only changes once they are verified against a fresh stack.
- [ ] Commit `iot` anonymous alignment separately if it lands cleanly.
- [ ] Commit `ros2` anonymous alignment separately if it lands cleanly.
- [ ] Push each verified batch to `origin/main`.
