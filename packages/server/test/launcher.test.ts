import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";

import {
  createHttpServerModuleTransport,
  createInMemoryServerModuleTransport,
  createNatsServerModuleTransport,
  createServerModuleLauncher,
  createServerModuleRegistry,
  discoverServerModules,
  startServerModules,
  subjectForServerModule,
} from "../src/index.ts";

async function createTempModulesDir() {
  const root = await mkdtemp(path.join(os.tmpdir(), "bladb-server-"));
  const modulesDir = path.join(root, "modules");
  await mkdir(modulesDir, { recursive: true });
  return { root, modulesDir };
}

test("discoverServerModules finds single-file ts modules by basename", async () => {
  const { modulesDir } = await createTempModulesDir();
  await writeFile(
    path.join(modulesDir, "user.ts"),
    [
      "export async function register(input) {",
      "  return { action: 'register', input };",
      "}",
      "export async function logout() {",
      "  return { revoked: true };",
      "}",
    ].join("\n"),
    "utf8",
  );

  const discovered = await discoverServerModules({ modulesDir });

  assert.equal(discovered.length, 1);
  assert.equal(discovered[0]?.moduleName, "user");
  assert.deepEqual(discovered[0]?.methods, ["logout", "register"]);
});

test("discoverServerModules rejects duplicate module basenames across extensions", async () => {
  const { modulesDir } = await createTempModulesDir();
  await writeFile(path.join(modulesDir, "user.ts"), "export async function login() {}", "utf8");
  await writeFile(path.join(modulesDir, "user.js"), "export async function logout() {}", "utf8");

  await assert.rejects(
    () => discoverServerModules({ modulesDir }),
    /duplicate server module name `user`/i,
  );
});

test("createServerModuleRegistry exposes only named function exports", async () => {
  const { modulesDir } = await createTempModulesDir();
  await writeFile(
    path.join(modulesDir, "user.ts"),
    [
      "export const value = 1;",
      "export default { skipped: true };",
      "export async function login(input) {",
      "  return { ok: true, input };",
      "}",
      "export function me() {",
      "  return { viewer: 'member' };",
      "}",
    ].join("\n"),
    "utf8",
  );

  const registry = await createServerModuleRegistry({ modulesDir });
  const methods = registry.listMethodsForModule("user");

  assert.deepEqual(methods, ["login", "me"]);
  const loginResult = await registry.invoke({
    app: "blog",
    module: "user",
    method: "login",
    input: { email: "editor@blog.demo" },
    requestId: "req_login",
    db: {
      user: {
        me() {
          return { viewer: "member" };
        },
      },
    },
  });
  assert.deepEqual(loginResult, { ok: true, input: { email: "editor@blog.demo" } });
});

test("subjectForServerModule builds app-qualified NATS subjects", () => {
  assert.equal(
    subjectForServerModule("blog", "user", "login"),
    "bladb.app.blog.module.user.login",
  );
});

test("server module functions can read request-scoped db without an explicit ctx argument", async () => {
  const { modulesDir } = await createTempModulesDir();
  const serverEntryUrl = pathToFileURL(path.resolve("D:\\study\\bladb\\packages\\server\\src\\index.ts")).href;
  await writeFile(
    path.join(modulesDir, "user.ts"),
    [
      `import { db } from ${JSON.stringify(serverEntryUrl)};`,
      "",
      "export async function me() {",
      "  return db.user.me();",
      "}",
    ].join("\n"),
    "utf8",
  );

  const registry = await createServerModuleRegistry({ modulesDir });
  const result = await registry.invoke({
    app: "blog",
    module: "user",
    method: "me",
    db: {
      user: {
        me() {
          return { uid: "anon_1", anonymous: true };
        },
      },
    },
  });

  assert.deepEqual(result, { uid: "anon_1", anonymous: true });
});

test("request-scoped db throws a clear error outside an active invocation", async () => {
  const { db } = await import("../src/index.ts");

  assert.throws(
    () => db.user.me(),
    /outside an active server module invocation/i,
  );
});

test("launcher subscribes one subject per module method and returns success envelopes", async () => {
  const { modulesDir } = await createTempModulesDir();
  await writeFile(
    path.join(modulesDir, "user.ts"),
    [
      "export async function login(input) {",
      "  return { ok: true, email: input.email };",
      "}",
      "export async function logout() {",
      "  return { revoked: true };",
      "}",
    ].join("\n"),
    "utf8",
  );

  const transport = createMemoryTransport();
  const launcher = await createServerModuleLauncher({
    app: "blog",
    modulesDir,
    transport,
  });

  const subscriptions = await launcher.start();
  assert.deepEqual(
    subscriptions,
    [
      "bladb.app.blog.module.user.login",
      "bladb.app.blog.module.user.logout",
    ],
  );

  const response = await transport.request("bladb.app.blog.module.user.login", {
    app: "blog",
    module: "user",
    method: "login",
    input: { email: "editor@blog.demo" },
    db: {},
    requestId: "req_2001",
  });

  assert.deepEqual(response, {
    ok: true,
    data: {
      ok: true,
      email: "editor@blog.demo",
    },
    requestId: "req_2001",
  });
});

test("launcher returns structured errors when a module handler throws", async () => {
  const { modulesDir } = await createTempModulesDir();
  await writeFile(
    path.join(modulesDir, "user.ts"),
    [
      "export async function login() {",
      "  throw new Error('bad credentials');",
      "}",
    ].join("\n"),
    "utf8",
  );

  const transport = createMemoryTransport();
  const launcher = await createServerModuleLauncher({
    app: "blog",
    modulesDir,
    transport,
  });

  await launcher.start();
  const response = await transport.request("bladb.app.blog.module.user.login", {
    app: "blog",
    module: "user",
    method: "login",
    input: { email: "editor@blog.demo" },
    db: {},
    requestId: "req_fail",
    meta: {
      traceId: "trace_fail",
    },
  });

  assert.deepEqual(response, {
    ok: false,
    error: {
      code: "SERVER_MODULE_ERROR",
      message: "bad credentials",
      module: "user",
      method: "login",
      traceId: "trace_fail",
    },
    requestId: "req_fail",
  });
});

test("startServerModules starts the launcher and exposes the registered subjects", async () => {
  const { modulesDir } = await createTempModulesDir();
  await writeFile(
    path.join(modulesDir, "user.ts"),
    [
      "export async function me() {",
      "  return { anonymous: true };",
      "}",
    ].join("\n"),
    "utf8",
  );

  const transport = createInMemoryServerModuleTransport();
  const started = await startServerModules({
    app: "blog",
    modulesDir,
    transport,
  });

  assert.deepEqual(started.subjects, ["bladb.app.blog.module.user.me"]);
  assert.deepEqual(transport.listSubjects(), ["bladb.app.blog.module.user.me"]);

  const response = await transport.request("bladb.app.blog.module.user.me", {
    app: "blog",
    module: "user",
    method: "me",
    db: {},
  });

  assert.deepEqual(response, {
    ok: true,
    data: {
      anonymous: true,
    },
  });
});

test("createNatsServerModuleTransport subscribes and responds with JSON envelopes", async () => {
  const fakeConnection = createFakeNatsConnection();
  const transport = await createNatsServerModuleTransport({
    servers: "nats://127.0.0.1:4222",
    async connectFn() {
      return fakeConnection;
    },
  });

  await transport.subscribe("bladb.app.blog.module.user.me", async (payload) => ({
    ok: true,
    payload,
  }));

  const response = await fakeConnection.dispatch("bladb.app.blog.module.user.me", {
    requestId: "req_nats",
    module: "user",
    method: "me",
  });

  assert.deepEqual(response, {
    ok: true,
    payload: {
      requestId: "req_nats",
      module: "user",
      method: "me",
    },
  });
});

test("createHttpServerModuleTransport exposes module handlers over local HTTP JSON", async () => {
  const transport = await createHttpServerModuleTransport({ port: 0, host: "127.0.0.1" });
  try {
    await transport.subscribe("bladb.app.blog.module.user.login", async (payload) => ({
      ok: true,
      payload,
    }));

    const baseUrl = transport.baseUrl();
    const response = await fetch(`${baseUrl}/invoke/bladb.app.blog.module.user.login`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
      },
      body: JSON.stringify({
        requestId: "req_http",
        module: "user",
        method: "login",
      }),
    });

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), {
      ok: true,
      payload: {
        requestId: "req_http",
        module: "user",
        method: "login",
      },
    });
  } finally {
    await transport.close();
  }
});

test("launcher exposes the raw invocation payload to modules that need token-based me flows", async () => {
  const { modulesDir } = await createTempModulesDir();
  await writeFile(
    path.join(modulesDir, "user.ts"),
    [
      "export async function me() {",
      "  return (globalThis as Record<string, unknown>).__bladbLauncherPayload;",
      "}",
    ].join("\n"),
    "utf8",
  );

  const transport = createInMemoryServerModuleTransport();
  const launcher = await createServerModuleLauncher({
    app: "blog",
    modulesDir,
    transport,
  });

  await launcher.start();
  const response = await transport.request("bladb.app.blog.module.user.me", {
    app: "blog",
    module: "user",
    method: "me",
    input: {},
    db: {
      user: {
        me: {
          token: "Bearer session-blog-1",
        },
      },
    },
  });

  assert.deepEqual(response, {
    ok: true,
    data: {
      app: "blog",
      module: "user",
      method: "me",
      input: {},
      db: {
        user: {
          me: {
            token: "Bearer session-blog-1",
          },
        },
      },
    },
  });
});

function createMemoryTransport() {
  return createInMemoryServerModuleTransport();
}

function createFakeNatsConnection() {
  const subscriptions = new Map<string, FakeSubscription>();

  return {
    subscribe(subject: string) {
      const subscription = new FakeSubscription();
      subscriptions.set(subject, subscription);
      return subscription;
    },
    async drain() {},
    async dispatch(subject: string, payload: unknown) {
      const subscription = subscriptions.get(subject);
      if (!subscription) {
        throw new Error(`missing fake NATS subscription for ${subject}`);
      }

      return await subscription.dispatch(payload);
    },
  };
}

class FakeSubscription {
  private readonly queue: FakeNatsMessage[] = [];
  private readonly waiters: Array<(value: IteratorResult<FakeNatsMessage>) => void> = [];

  [Symbol.asyncIterator]() {
    return {
      next: async () => {
        const nextMessage = this.queue.shift();
        if (nextMessage) {
          return { done: false, value: nextMessage };
        }

        return await new Promise<IteratorResult<FakeNatsMessage>>((resolve) => {
          this.waiters.push(resolve);
        });
      },
    };
  }

  async dispatch(payload: unknown) {
    const message = new FakeNatsMessage(payload);
    const waiter = this.waiters.shift();
    if (waiter) {
      waiter({ done: false, value: message });
    } else {
      this.queue.push(message);
    }

    return await message.response();
  }
}

class FakeNatsMessage {
  readonly data: Uint8Array;
  private resolved = false;
  private readonly responsePromise: Promise<unknown>;
  private resolveResponse!: (value: unknown) => void;

  constructor(payload: unknown) {
    this.data = new TextEncoder().encode(JSON.stringify(payload));
    this.responsePromise = new Promise((resolve) => {
      this.resolveResponse = resolve;
    });
  }

  respond(data: Uint8Array) {
    this.resolved = true;
    this.resolveResponse(JSON.parse(new TextDecoder().decode(data)));
  }

  async response() {
    const value = await this.responsePromise;
    if (!this.resolved) {
      throw new Error("fake NATS message did not receive a response");
    }

    return value;
  }
}
