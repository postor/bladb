# Standalone User Module V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a standalone, configuration-driven official `user module` that developers can use through `db.user` in JS and React, with a dedicated browser-verifiable demo and end-to-end login/logout flows.

**Architecture:** Treat `modules.official.users` as a real runtime assembly point instead of a docs-only contract. The gateway will assemble a `UserModuleProvider`, `PasswordService`, `SessionService`, and `MailerService` from config; the JS/React SDK will expose one stable `db.user` surface; and a dedicated `user-module-demo` app will validate the module in the browser without relying on IoT/ROS examples.

**Tech Stack:** Rust `bladb-gateway`, TypeScript `@bladb/client`, React `@bladb/react`, YAML config, Vite example app, local smoke/browser verification.

---

## Scope

### In scope

- standalone official user module runtime boundary
- config-driven provider assembly from `modules.official.users`
- login / register / me / logout flows
- JWT secret or key-file config support at the contract level
- password algorithm selection
- storage engine selection contract for `mysql` / `mongodb`
- mailer provider contract for `smtp`
- JS SDK `db.user.*`
- React SDK user session helper
- dedicated `apps/examples/user-module-demo`
- end-to-end browser verification

### Explicitly out of scope for V1

- full production-grade MySQL persistence implementation if adapter work becomes too large
- full production-grade MongoDB persistence implementation if adapter work becomes too large
- email verification / password reset execution flows
- cluster runtime deployment
- coupling verification to flash-sale / iot / ros2 business flows

## Delivery tracks

### Track 1: Server runtime

- formalize `OfficialUsersModuleConfig` validation and runtime assembly
- introduce provider boundary for user storage / auth operations
- wire login/register/me through official user module services
- define logout behavior clearly for standalone mode

### Track 2: SDK surface

- keep `db.user.login/register/me/logout` as the primary JS API
- add a React-first session helper around the official user module
- keep compatibility only where it does not obscure the official surface

### Track 3: Dedicated demo

- add `apps/examples/user-module-demo`
- page includes login, register, current session, logout
- page only demonstrates the user module, no unrelated business APIs

### Track 4: Verification

- Rust config/provider tests where environment allows
- JS/React tests for session behavior
- demo app build verification
- final browser verification in the in-app browser

## Subagent split

### Worker A: Server runtime

- ownership: `crates/bladb-gateway/src/local/*`, `crates/bladb-gateway/src/startup.rs`, related Rust tests
- responsibility: official user module runtime assembly, provider boundary, config-driven behavior

### Worker B: JS and React SDK

- ownership: `packages/client/*`, `packages/react/*`, related tests
- responsibility: official `db.user` and React session helpers

### Worker C: Dedicated demo app

- ownership: `apps/examples/user-module-demo/*`, root docs references if needed
- responsibility: standalone browser-verifiable user module UI

### Main agent

- integrate outputs
- resolve config/demo wiring
- run verification
- perform final browser test

## Verification gate

This work is not complete until all of the following are true:

- config can enable an official users module without relying on flash-sale / iot / ros2 app flows
- `db.user.login/register/me/logout` works in JS
- React session usage works against the same official module
- dedicated demo page works in the browser
- final browser validation proves login and logout visually
