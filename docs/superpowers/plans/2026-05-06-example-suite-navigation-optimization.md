# Example Suite Navigation Optimization Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the example apps into a cohesive suite by adding shared cross-example navigation, stable suite metadata, and docs that explain how to move through the demos.

**Architecture:** Keep each example app independent, but give them a shared browser-side metadata layer and a shared React navigation component. The stack scripts provide resolved example URLs so the same suite navigation works with default ports and auto-shifted ports.

**Tech Stack:** Vite React apps, shared example helper modules inside `apps/examples/shared`, stack URL env wiring in Node scripts and Docker compose, smoke/build/browser verification.

---

## Task Areas

### Task 1: Shared Suite Metadata And Navigation

**Files:**
- Create: `apps/examples/shared/exampleSuite.ts`
- Create: `apps/examples/shared/ExampleSuiteNav.tsx`
- Create: `apps/examples/shared/example-suite.css`

- [ ] Define one shared list of example apps with title, entry mode, summary, and resolved URL.
- [ ] Add a reusable navigation component that highlights the active app and links to the rest of the suite.
- [ ] Keep the component generic enough to reuse across all five examples.

### Task 2: Wire Navigation Into All Example Apps

**Files:**
- Modify: `apps/examples/flash-sale/src/App.tsx`
- Modify: `apps/examples/blog/src/App.tsx`
- Modify: `apps/examples/iot-realtime/src/App.tsx`
- Modify: `apps/examples/ros2-bridge/src/App.tsx`
- Modify: `apps/examples/user-module-demo/src/App.tsx`

- [ ] Mount the shared suite navigation near the top of each example page.
- [ ] Keep the current per-app visual language while making the suite rail feel consistent.

### Task 3: Provide Runtime-Aware Example URLs

**Files:**
- Modify: `scripts/dev-examples.mjs`
- Modify: `scripts/dev-examples-local.mjs`
- Modify: `docker/examples.compose.yaml`

- [ ] Pass resolved example URLs into each app so navigation stays correct when ports auto-shift.
- [ ] Keep local dev and Docker dev behavior aligned.

### Task 4: Docs And Verification

**Files:**
- Modify: `apps/examples/README.md`
- Modify: `README.md`

- [ ] Document the suite navigation and recommended traversal order.
- [ ] Run `pnpm build:examples`, `pnpm smoke:examples:local`, and browser spot checks after implementation.
