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

## 2026-05-04 - Real example configs should double as protocol fixtures

- Context: Worker and policy formats are easy to design in isolation but drift away from the examples users actually follow.
- Decision: Parse real example YAML files in core crate tests instead of relying only on synthetic test data.
- Why: It catches format drift early and keeps product docs, examples, and protocol code aligned.
- Apply when: Adding or changing worker manifests, policy manifests, and other user-authored config formats.
- Avoid when: The file under test is intentionally invalid or is testing only a narrow edge case.

## 2026-05-04 - Topic templates and key templates should share one value envelope

- Context: Redis keys and MQTT topics both need reserved values and request parameters inside native-looking client calls.
- Decision: Reuse the same serialized template envelope for keys and topics instead of inventing per-backend placeholder formats in the SDK payload.
- Why: It keeps the protocol smaller and lets the gateway resolve reserved values uniformly across data and stream modules.
- Apply when: Designing Redis key templates, MQTT topics, and other routed string targets with bound context.
- Avoid when: The backend target is always a literal string and gains nothing from template resolution.

## 2026-05-04 - Parseable protocol is not enough without operation validation

- Context: A request shape can deserialize cleanly while still describing nonsense like `kind=query engine=mqtt action=publish`.
- Decision: Add explicit protocol validation for supported `kind + engine + action` combinations and required fields.
- Why: It lets the gateway reject impossible requests before adapter code runs and keeps errors closer to the real contract boundary.
- Apply when: Adding new engines, actions, queue semantics, or stream behaviors.
- Avoid when: Testing raw serialization behavior in isolation from contract rules.

## 2026-05-04 - SQL should be classified by verb before policy matching

- Context: Treating every SQL statement as a generic query makes write policies awkward and forces the gateway to guess too late.
- Decision: Classify SQL requests by the leading statement verb into `select`, `insert`, `update`, or `delete` at the client/protocol boundary.
- Why: It gives policy matching cleaner semantics and keeps reads and writes distinct before execution logic is involved.
- Apply when: Serializing SQL requests, validating protocol combinations, and matching SQL policies.
- Avoid when: The backend interface already provides a stronger structured SQL operation model than raw statements.

## 2026-05-04 - Windows process wrappers need explicit shell handling for .cmd tools

- Context: Root dev orchestration scripts need to launch `pnpm` and other toolchain commands reliably on Windows while keeping Rust binaries direct.
- Decision: Spawn `.cmd` tools with shell support on Windows, but keep compiled executables like `bladb-gateway.exe` on direct spawn paths.
- Why: `pnpm.cmd` can fail with `spawn EINVAL` under direct spawn, while direct executables should avoid unnecessary shell wrapping.
- Apply when: Writing repo automation scripts that mix Node package manager commands with compiled binaries.
- Avoid when: The command is already a native executable path and does not require command-shell resolution.

## 2026-05-04 - Policy should map to logical module clusters before physical nodes

- Context: A distributed gateway needs to scale modules and shards without rewriting frontend calls or policy names.
- Decision: Bind each policy to one logical module cluster in topology config, then let routing choose service instance and shard from route keys.
- Why: This keeps policy stable while allowing `sql`, `redis`, `mongo`, `mqtt`, and worker runtimes to scale or split independently later.
- Apply when: Designing cluster topology, module discovery, route resolution, and dry-run tooling.
- Avoid when: A policy truly fans out to many clusters, in which case it should usually become an event or worker flow instead of a direct request path.

## 2026-05-04 - Example stacks should run through the same gateway config path as production

- Context: Hardcoded example assembly is convenient early on but makes the Rust side look disposable and teaches the wrong deployment shape.
- Decision: Move local auth users, runtime bindings, and in-memory module seeds into a gateway config file that the normal binary loads.
- Why: This keeps examples, smoke tests, and future production bootstrap on one path while still allowing local-only seed data.
- Apply when: Wiring gateway startup, adding new scenario modules, and documenting how example apps run.
- Avoid when: A test needs tiny inline fixtures and loading a file would obscure the behavior under test.

## 2026-05-05 - Unified startup should prefer one repo-level bladb.yml

- Context: Standalone and cluster startup were drifting toward separate file names and separate bootstrap conventions.
- Decision: Prefer one auto-discovered repo-level `bladb.yml`, with `mode: standalone` selecting the local single-binary path and non-standalone flows reading `runtime.role` before falling back to env.
- Why: This keeps startup compose-like for developers, avoids demo-only boot paths, and gives gateway/module/worker runtimes one config story to converge on.
- Apply when: Adding new startup modes, wiring example stacks, or extending runtime bootstraps beyond the gateway.
- Avoid when: A low-level runtime test needs a tiny dedicated fixture and full unified config loading would hide the behavior under test.

## 2026-05-05 - Privileged environment changes must be called out before execution

- Context: Toolchain, linker, Docker, and machine-level fixes can require administrator privileges or system-wide changes that affect more than the repo.
- Decision: If a command may need elevation or mutate machine-level configuration, tell the user first instead of retrying silently.
- Why: This keeps environment changes explicit, protects the machine state, and makes code issues easier to separate from local setup issues.
- Apply when: Installing toolchains, changing PATH, adding system dependencies, editing firewall or service settings, or running elevated package-manager commands.
- Avoid when: The command is clearly workspace-local and does not require elevation or system mutation.

## 2026-05-05 - Direct execution requests should not trigger redundant confirmation

- Context: During iterative build-out, the user often issues compact directives such as "continue", "start doing it", or names the exact implementation they want next.
- Decision: Treat those instructions as active permission to execute, and do not bounce back with "do you want me to proceed" unless there is a newly introduced hidden risk, destructive effect, or real fork in approach.
- Why: Re-asking after a clear instruction slows momentum and makes the agent feel unresponsive instead of collaborative.
- Apply when: Continuing implementation, refactoring, adding requested features, updating docs, or performing other normal workspace-local work already implied by the user's instruction.
- Avoid when: The next step would require elevation, destructive operations, system-wide mutation, or choosing among materially different paths the user has not accepted.

## 2026-05-05 - Cross-layer work should always keep a visible execution plan

- Context: Work that spans Rust runtime code, gateway wiring, Docker services, CLI behavior, and frontend validation is easy to lose track of when progress is only implicit.
- Decision: For non-trivial or multi-layer tasks, keep an explicit plan updated as steps complete or change.
- Why: A visible plan improves continuity, reduces repeated context recovery, and makes it easier to spot what remains versus what is already verified.
- Apply when: Debugging full-stack flows, implementing distributed features, refactoring across crates and apps, or coordinating code plus runtime verification.
- Avoid when: The task is a truly small single-step edit where a formal plan would add more noise than clarity.

## 2026-05-04 - Browser app modules should own auth persistence

- Context: Example frontends still had to wire `db.auth`, `sessionStore`, and `useGatewaySession(...)` by hand even after introducing module-level API clients.
- Decision: Treat browser auth persistence as part of the app module boundary, so one module object exposes `db`, `api`, and a session-aware `auth`.
- Why: This keeps frontend code focused on business flows, reduces repeated storage wiring, and makes example integrations closer to real production modules.
- Apply when: Designing browser-facing module SDK helpers, example app setup files, and future Vue or React module wrappers.
- Avoid when: The runtime is not browser-based and cannot rely on local session persistence.

## 2026-05-05 - Official module APIs should lead transport naming

- Context: The repo originally exposed session flows as `db.auth` because the standalone gateway transport was organized around `/auth/*`, while the desired long-term product shape is a first-class user module.
- Decision: Promote `db.user` as the official developer API first, while keeping `db.auth` and `/auth/*` as compatibility/backing transport during transitional versions.
- Why: Public module boundaries should reflect the product model, not temporary HTTP route naming. This keeps frontend usage stable while the server implementation grows into the official module contract.
- Apply when: Introducing official modules whose long-term API boundary is clearer than the current bootstrap implementation.
- Avoid when: The public API boundary is still highly uncertain and likely to churn immediately.

## 2026-05-05 - Official route aliases should be verified, not just documented

- Context: Adding `/users/*` as the public-facing transport alias for the official user module is easy to document, but easy to miss in automated verification if smoke tests only keep hitting `/auth/*`.
- Decision: When an official API or route alias is introduced, add it to smoke or end-to-end verification instead of relying on implementation review alone.
- Why: This catches drift between public contract, compatibility transport, and real runtime behavior before docs and code silently diverge.
- Apply when: Adding aliases, public compatibility routes, or new official module entrypoints.
- Avoid when: The route is intentionally private and not part of the supported developer-facing contract.

## 2026-05-05 - Realtime example credibility needs browser-visible proof

- Context: MQTT and ROS2 integrations can pass backend or protocol tests while still failing to demonstrate value clearly in the frontend.
- Decision: For realtime example features, require at least one browser-visible verification path that shows state changing from a live event, not only a successful publish call.
- Why: This validates the actual product story for frontend developers and catches UI/session/stream integration issues that backend tests miss.
- Apply when: Shipping example publish-subscribe flows, SSE-driven dashboards, or tenant-scoped live updates.
- Avoid when: The feature is intentionally backend-only and has no browser-facing contract.

## 2026-05-04 - Business commands should prefer module app APIs over frontend orchestration

- Context: The IoT example still made the browser assemble MQTT topics and actor payloads for a business action even though the Rust module already owned that workflow.
- Decision: Prefer module-owned `/apps/*` APIs for business commands such as device control, while keeping lower-level SQL, Mongo, Redis, or MQTT calls available for adapter-level use.
- Why: This keeps identity, tenant binding, topic construction, and policy-sensitive workflow details on the trusted side without making the frontend learn backend plumbing.
- Apply when: A frontend action maps to one business operation that should stay stable even if the underlying module protocol or routing details evolve.
- Avoid when: The user genuinely needs a low-level data or stream primitive rather than a business workflow.

## 2026-05-04 - Session-scoped dashboard reads can also be module-owned

- Context: The flash-sale example still made the browser assemble item, stock, wallet, and order reads separately even though the module already owned the business view.
- Decision: Allow modules to expose session-scoped summary endpoints such as `/apps/flash-sale/summary` for stable business reads, not only business writes.
- Why: This reduces round trips, keeps aggregation and user scoping on the trusted side, and makes frontend example code closer to the “just call the app API” adoption path.
- Apply when: A screen needs one coherent business snapshot that spans multiple backend primitives or store types.
- Avoid when: The caller explicitly needs raw low-level module access or independent cache lifetimes for each primitive.

## 2026-05-04 - Split runtimes need one shared internal bus contract

- Context: Once gateway, module runtimes, and worker runtimes are separated, ad-hoc JSON payloads between them become a long-term compatibility trap.
- Decision: Define shared internal bus envelopes in `bladb-core`, including gateway-to-module RPC requests and worker execution jobs/reports.
- Why: It keeps transport semantics stable across NATS request/reply and JetStream consumers while letting adapters and executors evolve behind one contract.
- Apply when: Designing module runtime handlers, worker dispatch loops, retries, dead letters, and internal observability payloads.
- Avoid when: The payload is purely local implementation detail and never crosses a runtime boundary.

## 2026-05-04 - Transport loops should sit on top of typed runners, not inside adapters

- Context: Going straight from NATS or JetStream callbacks into backend adapter code quickly tangles transport concerns with execution logic and makes tests brittle.
- Decision: Introduce runtime runners that drain typed inbox abstractions and delegate to already-validated runtime services.
- Why: This keeps the future live transport loop thin, makes unit tests work without a broker, and leaves one clear seam for swapping in real NATS / JetStream drivers.
- Apply when: Implementing request/reply serving, worker consumer polling, retries, and broker integration.
- Avoid when: The code path is purely in-process and never needs a broker boundary.

## 2026-05-06 - Server modules should separate request scope from transport delivery

- Context: The new `@bladb/server` package needs to let module authors write `db.user.me()` style code without exposing a `ctx` parameter, while still supporting NATS-backed delivery and future transport changes.
- Decision: Keep request-scoped `db` binding in one launcher/runtime layer and keep transport subscription behind a small interface that can be tested with an in-memory transport.
- Why: This preserves the desired developer ergonomics, keeps module code transport-agnostic, and lets the team validate module loading and invocation without depending on a live broker in every test.
- Apply when: Adding server-side module hosts, NATS adapters, worker launchers, or other request-scoped backend authoring surfaces.
- Avoid when: The code is purely synchronous utility logic with no request scope or transport boundary.
