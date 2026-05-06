# Anonymous Example Teaching Optimization Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the anonymous example pages up to the same developer-onboarding quality as the portal, blog, and user-module-demo so a frontend developer can open any official example and quickly understand the module path, SDK intent, and backend-owned responsibilities.

**Architecture:** Keep the existing anonymous example behavior intact, but enrich each page with browser-visible teaching surfaces: route ownership, expected SDK shape, and clear boundaries between browser input and trusted backend behavior. Update supporting docs so the suite description reflects that these are now example-teaching pages, not just runnable demos.

**Tech Stack:** React example apps, per-example CSS, markdown docs, existing example smoke/build pipeline, browser verification.

---

## Task Areas

### Task 1: Flash Sale Teaching Surfaces

**Files:**
- Modify: `apps/examples/flash-sale/src/App.tsx`
- Modify: `apps/examples/flash-sale/src/index.css`

- [ ] Add developer-facing route and queue-flow guidance to the flash-sale page.
- [ ] Surface a concise SDK-shaped example that explains how a frontend should think about the anonymous app API.
- [ ] Clarify which parts are browser-owned versus worker/backend-owned.

### Task 2: IoT And ROS2 Teaching Surfaces

**Files:**
- Modify: `apps/examples/iot-realtime/src/App.tsx`
- Modify: `apps/examples/iot-realtime/src/index.css`
- Modify: `apps/examples/ros2-bridge/src/App.tsx`
- Modify: `apps/examples/ros2-bridge/src/index.css`

- [ ] Make the IoT example explain read path, command publish path, and stream feedback path more explicitly.
- [ ] Make the ROS2 example explain publish, latest snapshot, and subscribe surfaces as a coherent module story.
- [ ] Add browser-visible SDK and backend-boundary guidance to both pages.

### Task 3: Docs Alignment

**Files:**
- Modify: `apps/examples/README.md`

- [ ] Update example-suite docs so the anonymous examples are described as teaching examples with explicit module-path guidance.

### Task 4: Verification

**Files:**
- Verify: `pnpm build:examples`
- Verify: `pnpm test:scripts`
- Verify: `pnpm smoke:examples:local`
- Verify: in-app browser checks for at least one updated anonymous example and the current ROS2 page

- [ ] Rebuild and rerun automation after the page upgrades.
- [ ] Confirm in the browser that the anonymous examples now communicate route ownership, SDK intent, and backend responsibilities more clearly.
