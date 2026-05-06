# Example Suite Portal Optimization Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a dedicated example-suite portal page so developers can start from one clear home, understand the recommended tour, see runtime URLs and seed credentials, and jump into any example app.

**Architecture:** Create a standalone Vite React app under `apps/examples/examples-portal` that uses the existing shared suite metadata and runtime-aware example URLs. Wire the portal into the same example stack scripts, Docker compose, build pipeline, and smoke checks as the other example apps.

**Tech Stack:** Vite React example app, shared example metadata/components, Node stack scripts, Docker compose, markdown docs, browser verification.

---

## Task Areas

### Task 1: Portal App

**Files:**
- Create: `apps/examples/examples-portal/package.json`
- Create: `apps/examples/examples-portal/index.html`
- Create: `apps/examples/examples-portal/src/main.tsx`
- Create: `apps/examples/examples-portal/src/App.tsx`
- Create: `apps/examples/examples-portal/src/index.css`

- [ ] Build a suite landing page with recommended traversal order, seed credentials, resolved URLs, and cards for all example apps.
- [ ] Keep the portal visual language intentional and distinct, while still matching the suite concept.

### Task 2: Stack Wiring

**Files:**
- Modify: `scripts/lib/example-stack.mjs`
- Modify: `scripts/dev-examples.mjs`
- Modify: `scripts/dev-examples-local.mjs`
- Modify: `scripts/smoke-examples.mjs`
- Modify: `scripts/smoke-examples-local.mjs`
- Modify: `docker/examples.compose.yaml`
- Modify: `docker/examples.dev.compose.yaml`
- Modify: `docker/examples.smoke.compose.yaml`
- Modify: `docker/frontend.Dockerfile`
- Modify: `package.json`

- [ ] Add a new stack slot for `examples-portal`.
- [ ] Pass the resolved portal URL and all example URLs into the frontend builds.
- [ ] Include the portal in build and smoke coverage.

### Task 3: Shared Metadata And Docs

**Files:**
- Modify: `apps/examples/shared/exampleSuite.ts`
- Modify: `apps/examples/README.md`
- Modify: `README.md`

- [ ] Extend suite metadata so the portal can be first-class in the developer journey.
- [ ] Update docs so the portal becomes the recommended starting point.

### Task 4: Verification

**Files:**
- Verify: `pnpm build:examples`
- Verify: `pnpm smoke:examples:local`
- Verify: in-app browser checks for the new portal plus at least one cross-link into an example

- [ ] Rebuild and rerun the example suite.
- [ ] Confirm the portal renders the right URLs and links into the active examples in the browser.
