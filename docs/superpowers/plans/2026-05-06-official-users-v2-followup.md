# Official Users Module V2 Follow-up Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the next layer of the official `db.user` story after the anonymous example-suite push by tightening developer onboarding, making browser verification reusable, and then moving the config contract toward real adapter-backed runtime execution.

**Architecture:** Keep the public frontend API stable around `db.user` and `createBrowserAppModule(...)`, but separate near-term work from larger runtime work. Near-term tasks should make the contract obvious and repeatable for developers; later tasks should replace the current local in-memory execution path with adapter-backed storage, crypto, and mailer assembly driven by `modules.official.users`.

**Tech Stack:** Markdown docs, YAML config examples, JS/React SDK docs, Node verification scripts, Rust local gateway/runtime code.

---

## What Is Already Done

- [x] `db.user` is the preferred JS/browser module surface.
- [x] `useUserSession(...)` / `useGatewaySession(...)` exist in `@bladb/react`.
- [x] Anonymous example flows now use cookie-backed identity with `db.user.me()` restoration.
- [x] `blog`, `examples-portal`, and `user-module-demo` give browser-visible teaching surfaces.
- [x] Example smoke covers auth aliases, anonymous flows, and `mongo + user`.

## Phase 1: Developer-Facing Contract Clarity

- [x] Add a single README section that answers:
  - where `modules.official.users` lives
  - how to choose `jwt.secret` vs `publicKeyFile/privateKeyFile`
  - how to choose `storage.engine=mysql` vs `mongodb`
  - when mailer config is required
  - how frontend developers should wire `db.user.login/register/me/logout`
- [x] Add a concrete config cookbook snippet in the config spec for:
  - HS256 + MySQL + SMTP
  - RS256 + MongoDB + SMTP
- [x] Keep the status note explicit that validation/config contract is live today, while full adapter-backed runtime execution is still a follow-up lane.

## Phase 2: Browser Verification As A Repo Primitive

- [x] Promote the ad hoc browser-visible checks into a committed script or test entrypoint.
- [x] Cover:
  - portal loads and teaches suite order
  - blog public read + editor publish
  - iot anonymous action feedback
  - ros2 anonymous publish/subscribe feedback
  - user-module-demo login -> refresh -> logout
- [x] Document where screenshots or browser artifacts should live and how developers rerun the flow locally.
- [ ] Extend smoke coverage so the mixed-auth `blog` example also proves anonymous cookie-backed identity and `db.user.me()` restoration, not just public reads plus editor writes.

## Phase 3: Official Users Runtime Assembly

- [ ] Audit the current `OfficialUserModule::from_config(...)` path and list which config fields only validate versus which fields actually drive runtime behavior.
- [ ] Introduce a provider split for:
  - session/token signing material
  - password hashing policy
  - storage backend selection
  - mail delivery backend
- [ ] Start with one real storage-backed path and one mailer-backed path rather than trying to land every adapter at once.
- [ ] Keep `gateway.auth.users` as the seeded local fixture while adding a migration path toward `modules.official.users.storage.*`.

## Phase 4: Runtime-Proving Tests

- [ ] Add Rust tests for config-to-runtime assembly boundaries.
- [ ] Add JS/browser tests that prove signed-out startup does not stay stuck in loading state.
- [ ] Add at least one end-to-end verification path for the first real storage-backed users runtime once Phase 3 starts landing.

## Suggested Execution Order

1. Finish docs/cookbook clarity first.
2. Check in reusable browser verification second.
3. Start the adapter-backed runtime path third.
4. Only then broaden into more providers or deeper auth features.
