# DB User Module V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the first official `db.user` module surface, keep example apps on one stable developer API, and define the standalone-to-future-production contract in config/docs.

**Architecture:** V1 keeps the current standalone auth/session runtime under the hood, but promotes `db.user` as the public module boundary. The gateway continues to back that surface with the local in-memory auth service and compatibility `/auth/*` transport while also exposing `/users/*` aliases and the declarative `modules.official.users` contract.

**Tech Stack:** TypeScript client and example apps, Rust `bladb-gateway`, YAML config/docs, Docker example stack, browser verification.

---

## Status Snapshot

### Delivered in V1

- [x] Plain `createClient(...)` instances expose `db.user.login/register/me`.
- [x] Browser app modules expose `.user` as the preferred surface and keep `.auth` as a compatibility alias.
- [x] Browser-managed `db.user.logout()` clears the stored token and session state.
- [x] Example apps now use `useGatewaySession(db.user)` instead of leading with `db.auth`.
- [x] Gateway routes expose `POST /users/login`, `POST /users/register`, and `GET /users/me` alongside `/auth/*`.
- [x] Local gateway app has explicit `user_login`, `user_register`, and `user_me` boundary methods over the auth service.
- [x] Config/docs define `modules.official.users` with session transport, JWT secret or key-file fields, password algorithm, MySQL or MongoDB storage, mailer settings, and feature flags.
- [x] Root docs point developers to the config spec so the official module contract is easy to find.
- [x] Smoke coverage verifies the public `/users/*` alias contract across `flash-sale`, `iot-realtime`, and `ros2-bridge`.

### Verification Completed

- [x] `node --experimental-strip-types --test packages/client/test/browser-module.test.ts`
  Passed with `7/7`, including `db.user` coverage plus keepalive/comment-frame SSE handling.
- [x] `pnpm build:examples`
  Passed in this workstream after the `db.user` and realtime changes landed.
- [x] `pnpm dev:examples`
  Rebuilt and started the current example stack successfully.
- [x] `pnpm smoke:examples:local`
  Passed, including `/users/*` alias checks and example app flows.
- [x] Browser verification
  - `flash-sale`: login works, logout returns to the login page, and no console errors/warnings were observed.
  - `iot-realtime`: `Reboot device` moves the UI from `connecting...` to `subscribed` and updates the last MQTT action/topic/delivery fields with no console errors/warnings.
  - `ros2-bridge`: the browser publish/subscribe flow was already verified earlier in this workstream.

### Known Constraint

- [ ] Host-native `cargo test -p bladb-gateway` is still not a reliable green signal on this Windows machine because the Rust/linker environment is unstable. V1 confidence currently comes from client tests, Docker rebuilds, smoke checks, and browser verification rather than a full local Rust test pass.

## Developer-Facing Usage

The intended usage now looks like this:

```ts
const session = await db.user.login({
  app: "flash-sale",
  email: "buyer@flash-sale.demo",
  password: "demo123"
});

await db.user.me();
```

Browser-managed module usage stays session-aware:

```ts
const flashSaleModule = createBrowserAppModule({
  appName: "flash-sale",
  baseUrl: "http://127.0.0.1:8787",
  tokenKey: "flash-sale.token",
  sessionKey: "flash-sale.session",
  routes: {}
});

await flashSaleModule.user.login({
  app: "flash-sale",
  email: "buyer@flash-sale.demo",
  password: "demo123"
});

flashSaleModule.db.user.logout();
```

Important transitional note:

- [x] `db.user` is the official public API.
- [x] `db.auth` still exists as a compatibility alias.
- [x] The plain client still transports these calls over `/auth/*` internally today.
- [x] `/users/*` exists as the public-facing alias transport and is covered by smoke verification.

## Implemented File Map

### Core implementation

- `D:\study\bladb\packages\client\src\index.ts`
- `D:\study\bladb\packages\client\test\browser-module.test.ts`
- `D:\study\bladb\crates\bladb-gateway\src\main.rs`
- `D:\study\bladb\crates\bladb-gateway\src\local\app.rs`
- `D:\study\bladb\crates\bladb-gateway\src\local\config.rs`
- `D:\study\bladb\crates\bladb-gateway\src\startup.rs`

### Docs and examples

- `D:\study\bladb\README.md`
- `D:\study\bladb\apps\docs\bladb-config-spec.md`
- `D:\study\bladb\bladb.yml`
- `D:\study\bladb\scripts\smoke-examples-local.mjs`
- `D:\study\bladb\LEARNINGS.md`

## Follow-Up Completed In This Batch

- [x] Rewrote this plan file into a status document instead of leaving it as an unchecked future plan.
- [x] Synced the root README with the official users-module entrypoint and manual verification notes.
- [x] Added missing realtime verification notes so developers can find the browser and CLI checks without hunting through session history.

## Remaining Work After V1

### Still worth doing soon

- [ ] Add reliable Rust-native gateway/runtime verification once the Windows toolchain is stable again.
- [ ] Decide when the plain client should switch its backing transport from `/auth/*` to `/users/*`, if at all.
- [ ] Re-run ROS2 browser verification after any future stream/runtime edits that affect subscription behavior.

### Explicit V2 backlog

- [ ] Wire `modules.official.users.storage` into real adapter-backed persistence.
- [ ] Implement JWT secret and key-file loading validation in the Rust runtime.
- [ ] Add password hashing strategy execution and migration handling.
- [ ] Add real mail delivery adapters for verification/reset flows.
- [ ] Separate local seeded standalone fixtures from production-shaped user-module state.

## Done Definition For V1

- [x] Developers can write `db.user.login/register/me`.
- [x] Browser-managed modules can write `db.user.logout()`.
- [x] Example apps use the official `db.user` surface.
- [x] Config/docs describe `modules.official.users` as an independent official module contract.
- [x] The repo has a verified bridge from current standalone auth runtime to the future user-module contract.
