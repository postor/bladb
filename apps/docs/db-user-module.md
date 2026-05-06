# Official `db.user` Module Guide

This guide is the shortest path to the official user-module contract in this repo.

Use it when you want to answer three practical questions:

1. How do I configure `modules.official.users`?
2. How should frontend code actually call `db.user.login/register/me/logout`?
3. Which parts are fully active today, and which parts are still contract-first?

## What `db.user` means

`db.user` is the preferred developer-facing session surface.

- New browser and JS examples should use `db.user`.
- `db.auth` still exists as a compatibility transport alias.
- New app code should prefer `db.user` unless you are deliberately working on old compatibility paths.

The server-side contract for this module lives under `modules.official.users`.

Related files:

- Repo config: [bladb.yml](/D:/study/bladb/bladb.yml)
- Config spec: [bladb-config-spec.md](/D:/study/bladb/apps/docs/bladb-config-spec.md)
- Dedicated demo: [user-module-demo](/D:/study/bladb/apps/examples/user-module-demo/src/App.tsx)

## Current status

The public API is real and in use today:

- `db.user.login(...)`
- `db.user.register(...)`
- `db.user.me()`
- `db.user.logout()` on browser-managed modules
- `useUserSession(...)` / `useGatewaySession(...)` in React

The config contract is also real today:

- startup validates `modules.official.users`
- the gateway carries the config into the local users module
- feature flags gate login/register behavior

Important honesty note:

- the current local runtime is still primarily backed by the local seeded-user/session path
- `jwt`, `password`, `storage`, and `mailer` fields are already modeled and validated
- full adapter-backed execution for MySQL / MongoDB / SMTP is still follow-up work, not fully shipped runtime behavior

## Server config cookbook

### Option A: HS256 + MySQL + SMTP

```yaml
official:
  users:
    enabled: true
    session:
      transport: gateway-auth
    jwt:
      algorithm: HS256
      secret: ${BLADB_JWT_SECRET}
    password:
      algorithm: argon2id
    storage:
      engine: mysql
      mysql:
        dsn: ${BLADB_USERS_MYSQL_DSN}
    mailer:
      provider: smtp
      from: no-reply@example.com
      smtp:
        host: smtp.example.com
        port: 587
        username: ${BLADB_SMTP_USER}
        password: ${BLADB_SMTP_PASS}
    features:
      login: true
      register: true
      verifyEmail: false
      resetPassword: false
```

Use this when:

- you want a shared-secret JWT setup
- your future persistence target is MySQL
- you only need login/register first

### Option B: RS256 + MongoDB + SMTP

```yaml
official:
  users:
    enabled: true
    session:
      transport: gateway-auth
    jwt:
      algorithm: RS256
      publicKeyFile: ./keys/users.public.pem
      privateKeyFile: ./keys/users.private.pem
    password:
      algorithm: bcrypt
    storage:
      engine: mongodb
      mongodb:
        uri: ${BLADB_USERS_MONGODB_URI}
        database: bladb_users
    mailer:
      provider: smtp
      from: no-reply@example.com
      smtp:
        host: smtp.example.com
        port: 587
        username: ${BLADB_SMTP_USER}
        password: ${BLADB_SMTP_PASS}
    features:
      login: true
      register: true
      verifyEmail: true
      resetPassword: true
```

Use this when:

- you want asymmetric signing keys instead of a shared secret
- your future persistence target is MongoDB
- you plan to turn on email-driven flows

## Frontend usage

### Plain JS client

```ts
import { createClient } from "@bladb/client";

const db = createClient({ baseUrl: "http://127.0.0.1:8787" });

const session = await db.user.login({
  app: "blog",
  email: "editor@blog.demo",
  password: "demo123"
});

await db.user.me();
```

Use this when you want transport-level auth calls without browser-managed persistence.

### Browser-managed module

```ts
import { createBrowserAppModule } from "@bladb/client";

const blogModule = createBrowserAppModule({
  baseUrl: "http://127.0.0.1:8787",
  appName: "blog",
  tokenKey: "bladb.blog.token",
  sessionKey: "bladb.blog.session",
  routes: {}
});

await blogModule.db.user.login({
  app: "blog",
  email: "editor@blog.demo",
  password: "demo123"
});

await blogModule.db.user.me();
await blogModule.db.user.logout();
```

Use this when the browser should own token/session persistence.

### React usage

```tsx
import { useUserSession } from "@bladb/react";

function BlogAuthPanel() {
  const session = useUserSession(blogModule.user);

  return (
    <button
      onClick={() =>
        session.login({
          app: "blog",
          email: "editor@blog.demo",
          password: "demo123"
        })
      }
    >
      Login
    </button>
  );
}
```

Prefer `useUserSession(...)` when your screen is specifically about `db.user`.

`useGatewaySession(...)` still works and is fine when you are already thinking in broader gateway session terms, but the user-focused hook is the clearer default for new auth-centric screens.

## What to use today

- Prefer `db.user` over `db.auth` in new code.
- Prefer `createBrowserAppModule(...)` for browser apps that need persisted sessions.
- Prefer `useUserSession(...)` for React auth/UI work.
- Use `gateway.auth.users` for seeded local fixtures.
- Treat `modules.official.users` as the long-term server contract you should design around.

## Where to see it working

- [apps/examples/user-module-demo](/D:/study/bladb/apps/examples/user-module-demo/src/App.tsx)
- [apps/examples/blog](/D:/study/bladb/apps/examples/blog/src/App.tsx)
- [README.md](/D:/study/bladb/README.md)
