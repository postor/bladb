# Example Experience Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Polish the anonymous example suite and the new `blog` demo so developers can understand the intended module usage immediately and complete the core flows with less friction.

**Architecture:** Keep the runtime/module contract unchanged and focus this pass on frontend clarity, auth-state ergonomics, and example documentation. The `blog` app remains the mixed public-read plus authenticated-editor demo, while the anonymous examples continue to foreground direct entry and seeded identities.

**Tech Stack:** Vite React example apps, `@bladb/react`, `@bladb/client`, markdown docs, local smoke/browser verification.

---

## Optimization Targets

### Track 1: Blog Editor UX

**Files:**
- Modify: `apps/examples/blog/src/App.tsx`
- Modify: `apps/examples/blog/src/index.css`
- Modify: `apps/examples/blog/src/bladb.ts`

- [ ] Make signed-out and signed-in editor states clearer, including when logout is meaningful.
- [ ] Add a compact “how this demo works” summary so users can see public-read vs editor-write responsibilities at a glance.
- [ ] Surface better publish feedback so a new post is visibly confirmed in both the editor feed and the public feed.

### Track 2: Anonymous Example Consistency

**Files:**
- Modify: `apps/examples/flash-sale/src/App.tsx`
- Modify: `apps/examples/iot-realtime/src/App.tsx`
- Modify: `apps/examples/ros2-bridge/src/App.tsx`

- [ ] Add a lightweight “module path” or “what this page demonstrates” explanation to each anonymous example.
- [ ] Tighten copy so seeded identity behavior, worker/background behavior, and app API behavior are easier to understand.

### Track 3: Example Docs

**Files:**
- Modify: `apps/examples/README.md`
- Modify: `README.md`

- [ ] Document the entry mode and primary learning goal for each example.
- [ ] Call out which examples are anonymous, which use `db.user`, and why that split exists.

### Track 4: Verification

**Files:**
- Verify: `pnpm --dir apps/examples/blog build`
- Verify: `pnpm build:examples`
- Verify: `pnpm smoke:examples:local`
- Verify: in-app browser checks for `blog`, `flash-sale`, `iot-realtime`, `ros2-bridge`, `user-module-demo`

- [ ] Run fresh build and smoke verification after the polish pass.
- [ ] Re-test the browser flows to confirm the UX changes did not regress anonymous entry or editor auth flows.
