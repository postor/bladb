# Anonymous Identity And Flash Sale Collaboration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add cookie-backed anonymous identities with renewable leases and `me` support, then use those identities to make the flash-sale example behave like a coordinated `db + redis + worker` flow instead of a hardcoded guest fallback.

**Architecture:** The gateway will resolve sessions from either bearer tokens or a first-party cookie. When an anonymous app request arrives for a module that allows anonymous access, the official user module will mint a real session-backed identity for that app, persist it in a cookie, renew its lease on later requests, and expose the same identity through `db.user.me()`. Flash-sale will stop using a static guest user and instead run queue, reservation, order, and projection state against that anonymous identity while modeling separate db, redis, and worker responsibilities inside the local runtime.

**Tech Stack:** Rust local gateway runtime, `@bladb/client`, `@bladb/react`, Vite React example app, Node/browser tests.

---

## Phase 1: Session Transport

- [ ] Extend the in-memory auth service with session kinds, lease timestamps, anonymous identity minting, cookie lookup, and renewal helpers.
- [ ] Extend the official user module so the gateway can resolve `me`, logout, and app sessions from bearer or cookie transport.
- [ ] Teach the standalone HTTP gateway to read cookies, write `Set-Cookie`, and pass cookie-aware session context into app APIs and `/users/me`.

## Phase 2: SDK And Session UX

- [ ] Update `@bladb/client` browser requests to include credentials so cookie-backed sessions survive reloads.
- [ ] Relax browser session refresh so `me()` can hydrate from cookies even when there is no local bearer token.
- [ ] Keep the existing authenticated flows working for the dedicated user-module demo.

## Phase 3: Flash Sale Runtime

- [ ] Replace the static flash-sale guest identity with the resolved anonymous or authenticated session.
- [ ] Split flash-sale state into db-like order/event storage, redis-like hot counters and queue tickets, and worker-like asynchronous settlement.
- [ ] Ensure anonymous identities receive stable wallet, queue, and order views across refreshes until lease expiry.

## Phase 4: Example App And Verification

- [ ] Update the flash-sale example UI to surface the current anonymous identity and the db/redis/worker collaboration path.
- [ ] Add Rust and TypeScript coverage for anonymous session minting, renewal, `me`, logout, and flash-sale ownership isolation.
- [ ] Run targeted tests plus browser verification against the local example stack.
