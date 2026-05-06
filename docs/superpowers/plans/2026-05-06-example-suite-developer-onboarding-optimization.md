# Example Suite Developer Onboarding Optimization Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the official example suite easier for developers to understand and adopt by clarifying the learning path, surfacing module usage intent, and improving how the portal and example pages explain what each demo proves.

**Architecture:** Extend the shared example metadata so the suite can describe learning stage, module stack, and concrete developer takeaways in one place. Reuse that richer metadata across the portal and shared navigation, then strengthen the `blog` and `user-module-demo` pages with guidance that connects browser behavior to the intended `db.user` and `db.mongo` usage model.

**Tech Stack:** Shared TypeScript metadata, React example apps, shared CSS, markdown plans/docs, local/browser verification, example smoke/build checks.

---

## Task Areas

### Task 1: Shared Suite Learning Metadata

**Files:**
- Modify: `apps/examples/shared/exampleSuite.ts`
- Modify: `apps/examples/shared/ExampleSuiteNav.tsx`
- Modify: `apps/examples/shared/example-suite.css`

- [ ] Add richer shared metadata for learning stage, module stack, and developer takeaway.
- [ ] Update shared navigation to show clearer suite progression instead of acting like a flat link list.
- [ ] Preserve resolved runtime URLs while making the suite order easier to scan.

### Task 2: Portal As Developer Start Page

**Files:**
- Modify: `apps/examples/examples-portal/src/App.tsx`
- Modify: `apps/examples/examples-portal/src/index.css`

- [ ] Reframe the portal around “where to start”, “what each demo teaches”, and “which modules are being exercised”.
- [ ] Add a stronger guided-tour surface that helps a developer choose the next example intentionally.
- [ ] Keep the portal compatible with resolved dynamic example URLs.

### Task 3: Deepen Blog And User Module Guidance

**Files:**
- Modify: `apps/examples/blog/src/App.tsx`
- Modify: `apps/examples/blog/src/index.css`
- Modify: `apps/examples/user-module-demo/src/App.tsx`
- Modify: `apps/examples/user-module-demo/src/index.css`

- [ ] Make `blog` explain the public-read plus authenticated-write split more explicitly.
- [ ] Make `user-module-demo` explain how a developer would actually use `db.user.login`, `register`, `me`, and `logout` in an app.
- [ ] Add concise browser-visible guidance that ties UI behavior back to the intended module contract.

### Task 4: Verification

**Files:**
- Verify: `pnpm build:examples`
- Verify: `pnpm test:scripts`
- Verify: `pnpm smoke:examples:local`
- Verify: in-app browser checks for portal plus at least one example page

- [ ] Rebuild the example apps after the onboarding updates.
- [ ] Re-run automation to ensure suite metadata and runtime link behavior still hold.
- [ ] Verify in the browser that the portal and example pages now better communicate the intended learning path.
