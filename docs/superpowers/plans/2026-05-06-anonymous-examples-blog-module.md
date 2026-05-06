# Anonymous Examples And Blog Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the functional example apps to anonymous direct-entry mode, remove their login gates, then start a new `blog` example built on `mongo + user`, with tests, scripts, docs, and browser verification.

**Architecture:** Keep the official `db.user` module as the long-term auth contract, but let example apps opt into anonymous app access by falling back to seeded default identities inside the local gateway runtime. Frontend examples should use optional-auth typed app clients or optional-auth execute clients when the UX is intentionally direct-entry.

**Tech Stack:** Rust local gateway runtime, `@bladb/client`, `@bladb/react`, Vite React examples, Node smoke scripts, in-app browser verification.

---

## Phase Breakdown

### Phase 1: Planning And Control Surface

- [x] Update `AGENT.md` so agents must write repo-visible plans, continue decomposing unfinished work, and include browser verification when requested.
- [x] Save this plan under `docs/superpowers/plans/2026-05-06-anonymous-examples-blog-module.md`.
- [ ] Keep the phase checklist updated as implementation lands.

### Phase 2: Anonymous Example Runtime Support

**Files:**
- Modify: `crates/bladb-gateway/src/local/app.rs`
- Modify: `crates/bladb-gateway/src/local/flash_sale.rs`
- Modify: `crates/bladb-gateway/src/local/ros2.rs`
- Modify: `bladb.yml`
- Test: Rust module tests in `crates/bladb-gateway/src/local/ros2.rs`
- Test: add flash-sale local module tests in `crates/bladb-gateway/src/local/flash_sale.rs`

- [ ] Add anonymous app-access flags and guest identity fallback for `flash-sale` and `ros2-bridge`, matching the existing IoT pattern.
- [ ] Allow `/apps/ros2-bridge/.../stream` to open without bearer auth when anonymous app access is enabled.
- [ ] Add or extend Rust tests covering anonymous app API access and anonymous stream access.

### Phase 3: Anonymous Example Frontends

**Files:**
- Modify: `apps/examples/flash-sale/src/bladb.ts`
- Modify: `apps/examples/flash-sale/src/App.tsx`
- Modify: `apps/examples/ros2-bridge/src/bladb.ts`
- Modify: `apps/examples/ros2-bridge/src/App.tsx`
- Review: `apps/examples/iot-realtime/src/App.tsx`

- [ ] Switch example API clients to optional-auth guest clients where the UI should open directly.
- [ ] Remove login and register screens from `flash-sale` and `ros2-bridge`.
- [ ] Replace session-dependent copy with explicit anonymous-example copy that explains the seeded identity in use.
- [ ] Keep `user-module-demo` as the dedicated auth showcase and leave IoT aligned with anonymous mode.

### Phase 4: Blog Example Bootstrap (`mongo + user`)

**Files expected:**
- Create: `apps/examples/blog/*`
- Modify: `bladb.yml`
- Modify: `package.json`
- Modify: `scripts/lib/example-stack.mjs`
- Modify: `scripts/dev-examples*.mjs` and smoke/build wiring as needed
- Modify: `apps/examples/README.md`

- [ ] Inspect the current local Mongo runtime path and choose the smallest viable blog slice.
- [ ] Scaffold a `blog` example with at least article list, article create, and user-aware ownership flow.
- [ ] Wire the example into local config, scripts, build, and smoke flows.

### Phase 5: Verification And Browser Proof

**Files:**
- Modify: `scripts/smoke-examples-local.mjs`
- Modify: any supporting docs or scripts discovered during implementation

- [ ] Keep `/users/*` alias verification in smoke because `db.user` remains the official module contract.
- [ ] Add anonymous app API verification for `flash-sale` and `ros2-bridge`.
- [ ] Add `blog` smoke coverage once the example exists.
- [ ] Run targeted Rust tests, example build/tests, and in-app browser verification for the changed flows.

## Active Execution Notes

- The main critical path starts with runtime anonymous-access support because the frontend de-login work depends on it.
- Parallel scouting is allowed for non-blocking tasks such as `blog` example landing shape and script/doc impact.
- `user-module-demo` remains the single auth-focused example; the other functional examples should teach business APIs without forcing sign-in first.
