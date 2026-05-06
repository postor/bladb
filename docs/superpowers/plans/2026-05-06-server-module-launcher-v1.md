# Server Module Launcher V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first usable `@bladb/server` launcher that scans single-file JS or TS modules, exposes named async exports over a transport abstraction, and binds a request-scoped `db` object with tests.

**Architecture:** Add a new workspace package at `packages/server`. The package owns module discovery, request-scoped `db`, registry creation, and transport-backed launcher startup. The first transport implementation targets NATS, but tests use an in-memory transport so the module host can be validated without a broker.

**Tech Stack:** TypeScript, Node built-in test runner, AsyncLocalStorage, dynamic ESM import, NATS client for Node, Markdown specs and plans.

---

## File Map

- Create: `packages/server/package.json`
- Create: `packages/server/tsconfig.json`
- Create: `packages/server/src/index.ts`
- Create: `packages/server/src/db.ts`
- Create: `packages/server/src/discovery.ts`
- Create: `packages/server/src/launcher.ts`
- Create: `packages/server/src/nats.ts`
- Create: `packages/server/test/launcher.test.ts`
- Modify: `package.json`
- Modify: `README.md`

## Task 1: Package Skeleton And Discovery Tests

**Files:**

- Create: `packages/server/package.json`
- Create: `packages/server/tsconfig.json`
- Create: `packages/server/test/launcher.test.ts`
- Modify: `package.json`

- [ ] Add the new workspace package metadata and a root `test:server` script.
- [ ] Write failing tests for:
  - discovering `user.ts`
  - rejecting duplicate `user.ts` plus `user.js`
  - exposing only named function exports
- [ ] Run `pnpm test:server` and confirm the new tests fail because the package implementation does not exist yet.

## Task 2: Discovery And Registry Minimum

**Files:**

- Create: `packages/server/src/discovery.ts`
- Create: `packages/server/src/launcher.ts`
- Create: `packages/server/src/index.ts`
- Modify: `packages/server/test/launcher.test.ts`

- [ ] Implement module directory scanning for `.ts`, `.mts`, `.js`, and `.mjs`.
- [ ] Implement duplicate module-name detection across mixed extensions.
- [ ] Implement dynamic import plus named async export extraction.
- [ ] Run `pnpm test:server` and confirm the discovery tests pass.

## Task 3: Request-Scoped `db`

**Files:**

- Create: `packages/server/src/db.ts`
- Modify: `packages/server/src/index.ts`
- Modify: `packages/server/test/launcher.test.ts`

- [ ] Write failing tests that call a module function using `import { db } from "@bladb/server"` and verify request-local methods resolve correctly.
- [ ] Add a failing test that calling `db.user.me()` outside a launcher invocation throws a clear error.
- [ ] Implement AsyncLocalStorage-backed request scope binding.
- [ ] Run `pnpm test:server` and confirm all scope tests pass.

## Task 4: Transport Launcher

**Files:**

- Modify: `packages/server/src/launcher.ts`
- Modify: `packages/server/test/launcher.test.ts`

- [ ] Write failing tests for:
  - subject naming
  - transport subscription per discovered method
  - success response envelope
  - structured error response envelope
- [ ] Implement the transport abstraction and launcher startup path.
- [ ] Run `pnpm test:server` and confirm the launcher tests pass.

## Task 5: NATS Adapter

**Files:**

- Create: `packages/server/src/nats.ts`
- Modify: `packages/server/src/index.ts`

- [ ] Add the Node NATS dependency.
- [ ] Implement a NATS request/reply adapter that decodes JSON, delegates to the launcher handler, and replies with JSON.
- [ ] Add a narrow unit test for adapter payload encoding if feasible without a live broker; otherwise ensure the code typechecks and is exercised in a follow-up integration batch.

## Task 6: Docs And Entry Surface

**Files:**

- Modify: `README.md`
- Modify: `packages/server/src/index.ts`

- [ ] Export the public `@bladb/server` surface from one entrypoint.
- [ ] Add a concise README section showing `startServerModules(...)` and `user.ts`.
- [ ] Re-run `pnpm test:server`.

## Task 7: Next Integration Queue

**Files:**

- None in this batch unless small notes are needed in docs.

- [ ] After the JS launcher is green, start the Rust follow-up batch:
  - add gateway config for server-module launcher transport
  - add Rust NATS client wiring for official `user` methods
  - add browser-visible example verification through the launcher-backed path
