# Learnings

## 2026-05-04 - Reserved values are protocol-level concepts

- Context: Frontend calls should look native while backend policy stays strict and fast.
- Decision: `UID`, `TENANT_ID`, `ROLES`, and `PERMISSION_VERSION` are treated as reserved protocol values instead of plain user input.
- Why: This keeps JWT mapping, policy injection, and audit semantics stable across SQL, Mongo, Redis, and future modules.
- Apply when: Designing client payloads, gateway request models, and policy compilers.
- Avoid when: A value is app-specific and should remain regular user data.

## 2026-05-04 - Cross-module changes should prefer event plus worker

- Context: Orders, telemetry, alerts, and delayed jobs all need follow-up work across multiple backends.
- Decision: Prefer `primary command -> event -> worker` over direct hidden module-to-module calls.
- Why: It improves auditability, retry handling, idempotency, and long-term extensibility.
- Apply when: A successful operation needs analytics, notification, projection, timeout, or compensation work.
- Avoid when: The work must be completed synchronously before the request can safely return.

## 2026-05-04 - Optimal-first beats compatibility-first

- Context: Early-stage platform architecture is easier to improve now than after adapters and users depend on rough edges.
- Decision: Default to the best long-term design unless the user explicitly requests compatibility constraints.
- Why: Pre-1.0 systems gain more from clean foundations than from preserving accidental early APIs.
- Apply when: Choosing protocol shapes, policy models, worker contracts, and module boundaries.
- Avoid when: A compatibility requirement is explicit, contractual, or externally deployed already.
