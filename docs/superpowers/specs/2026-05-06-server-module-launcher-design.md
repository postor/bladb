# Server Module Launcher Design

## Goal

Add a first-class `@bladb/server` package that lets backend developers write server modules as JS or TS files, expose named async exports such as `register`, `login`, `me`, and `logout`, and have those methods served over NATS by a launcher process. The launcher becomes the shared execution model for the official `user` module and future third-party modules.

## Non-Goals For V1

- Replace every existing Rust local module in one change.
- Ship a full storage-backed official user runtime in this same batch.
- Support multi-file directory modules on day one.
- Embed a JS runtime directly inside Rust on the first pass.

## Developer Experience

Backend developers should be able to author one file per module:

```ts
// app/modules/user.ts
import { db } from "@bladb/server";

export async function register(input) {
  const passwordHash = await db.user.password.hash(input.password);
  const user = await db.mongo.users.insertOne({
    email: input.email,
    displayName: input.displayName,
    passwordHash,
  });

  return db.user.session.issue({
    uid: user.uid,
    roles: ["member"],
  });
}

export async function login(input) {
  const user = await db.mongo.users.findOne({ email: input.email });
  await db.user.password.verify(input.password, user.passwordHash);
  return db.user.session.issue({
    uid: user.uid,
    roles: user.roles ?? ["member"],
  });
}

export async function me() {
  return db.user.me();
}

export async function logout() {
  await db.user.session.revokeCurrent();
  return { revoked: true };
}
```

The server package API should stay close to the client package:

- client: `import { db } from "@bladb/client"`
- server: `import { db, startServerModules } from "@bladb/server"`

The mental model should match even when the execution model differs.

## Architecture

V1 uses a separate Node launcher instead of embedding JS into Rust:

1. `@bladb/server` starts a launcher process.
2. The launcher scans a configured module directory for single-file modules such as `user.ts`.
3. Each filename becomes the module name.
4. Each named async export becomes a callable method.
5. The launcher subscribes to one NATS subject per `app.module.method`.
6. Rust gateway or future callers send a request envelope over NATS request/reply.
7. The launcher binds request-scoped capabilities to the exported `db` object for the duration of that method call.
8. The launcher returns JSON-safe success or structured error payloads.

## Module Discovery Rules

V1 discovery rules:

- Scan one configured directory only.
- Accept `.ts`, `.mts`, `.js`, and `.mjs`.
- Ignore files whose basenames start with `_`.
- Use the basename without extension as the module name.
- Reject duplicate module names across mixed extensions.
- Only named function exports are exposed as callable methods.
- Default exports are ignored in V1.

V1 intentionally does not support directory modules such as `user/index.ts`.

## Request Envelope

NATS payloads should use one stable envelope:

```json
{
  "app": "blog",
  "module": "user",
  "method": "login",
  "requestId": "req_123",
  "input": {
    "email": "editor@blog.demo",
    "password": "demo123"
  },
  "db": {
    "user": {},
    "mongo": {},
    "mysql": {},
    "redis": {},
    "mailer": {},
    "worker": {}
  },
  "identity": {
    "uid": null,
    "tenantId": "tenant_blog",
    "roles": ["anonymous"],
    "anonymous": true
  },
  "meta": {
    "traceId": "trace_123"
  }
}
```

The launcher owns only method dispatch and request-scoped binding. It does not own authentication policy. Rust still owns auth, route selection, and trusted capability assembly.

## Request-Scoped `db`

The server package must not require a visible `ctx` parameter. Instead it should expose a request-local `db` handle implemented with invocation scope storage.

Rules:

- `db` exists as a package-level export.
- Every incoming request gets its own scope.
- Module code can call `db.user.me()` or `db.mongo.users.findOne(...)` directly.
- A call outside an active invocation fails fast with a clear error.
- Nested async calls within one invocation must keep the same scope.

## Subject Naming

Use app-qualified subjects:

- `bladb.app.blog.module.user.register`
- `bladb.app.blog.module.user.login`
- `bladb.app.blog.module.user.me`
- `bladb.app.blog.module.user.logout`

This keeps multi-app isolation explicit and avoids collisions when many apps have a `user.ts`.

## V1 Launcher Surface

`@bladb/server` should expose:

- `db`
- `discoverServerModules(options)`
- `createServerModuleRegistry(options)`
- `createServerModuleLauncher(options)`
- `startServerModules(options)`
- `subjectForServerModule(app, module, method)`

The launcher should also support a small transport abstraction so tests can verify behavior without a live NATS server.

## NATS Responsibilities

The launcher transport layer should:

- connect to one NATS server URL
- subscribe per discovered subject
- decode JSON request envelopes
- invoke the registry handler
- reply with JSON `{ ok: true, data }` or `{ ok: false, error }`
- preserve `requestId` and `traceId` in error payloads when present

The Rust side will later use the same subject contract.

## V1 Rust Integration Target

The first Rust integration should not replace all gateways. It should add one new path that can call a server module over NATS for the official `user` contract:

- `db.user.register`
- `db.user.login`
- `db.user.me`
- `db.user.logout`

The existing local in-memory provider remains as fallback while the NATS-backed path is introduced behind config.

## Testing

V1 must include:

- JS tests for module discovery rules
- JS tests for duplicate module rejection
- JS tests for method registry creation
- JS tests for request-scoped `db`
- JS tests for launcher transport subscription using an in-memory transport
- JS tests for structured error responses

Follow-up tests after Rust wiring lands:

- Rust tests for NATS envelope construction
- Rust tests for gateway user-module dispatch
- browser-visible user flow through the new launcher-backed path

## Rollout

Phase 1:

- add `@bladb/server`
- ship launcher, registry, request-scoped `db`, and NATS transport
- keep tests fully in JS with in-memory transport coverage

Phase 2:

- add Rust gateway NATS client for official `user` module methods
- make the launcher runnable in local dev

Phase 3:

- switch one example app to the launcher-backed official user module
- add browser verification

## Key Tradeoff

This design chooses a separate launcher process instead of immediate Rust-embedded JS execution.

Why:

- better TS ergonomics now
- lower implementation risk for the first usable version
- natural fit for NATS-based module RPC
- stronger path toward third-party module authoring

Avoid when:

- the module must run in-process with the Rust gateway for ultra-low-latency compute
- the workload is mostly pure computation rather than request-driven app logic
