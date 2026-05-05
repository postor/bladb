import assert from "node:assert/strict";
import test from "node:test";
import {
  createUserModuleDemoModule,
  describeSessionFacts,
  describeSessionEnvelope,
  describeVerificationChecklist,
  type UserModuleDemoSession
} from "./bladb.ts";

class MemoryStorage {
  private readonly values = new Map<string, string>();

  clear() {
    this.values.clear();
  }

  getItem(key: string) {
    return this.values.get(key) ?? null;
  }

  removeItem(key: string) {
    this.values.delete(key);
  }

  setItem(key: string, value: string) {
    this.values.set(key, value);
  }
}

function installWindow() {
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      localStorage: new MemoryStorage()
    }
  });
}

function jsonResponse<T>(status: number, data: T): Response {
  return new Response(JSON.stringify({ data }), {
    status,
    headers: {
      "content-type": "application/json"
    }
  });
}

function demoSession(overrides: Partial<UserModuleDemoSession["user"]> = {}): UserModuleDemoSession {
  return {
    token: "token_user_module_demo",
    user: {
      app: "user-module-demo",
      uid: "u_4001",
      tenantId: "tenant_local",
      email: "person@example.com",
      displayName: "Demo Person",
      roles: ["member"],
      ...overrides
    }
  };
}

test("user module demo module persists a registered session and refreshes current user details", async () => {
  installWindow();

  const session = demoSession();
  const requests: Array<{ url: string; method: string; headers: Headers }> = [];
  const userModule = createUserModuleDemoModule({
    baseUrl: "http://localhost:8787",
    fetcher: async (input, init) => {
      const url = String(input);
      const method = init?.method ?? "GET";
      const headers = new Headers(init?.headers);
      requests.push({ url, method, headers });

      const path = new URL(url).pathname;
      if (path === "/users/register") {
        return jsonResponse(200, session);
      }

      if (path === "/users/login") {
        return jsonResponse(200, session);
      }

      if (path === "/users/me") {
        return jsonResponse(200, session);
      }

      if (path === "/users/logout") {
        return jsonResponse(200, {
          loggedOut: true
        });
      }

      throw new Error(`Unexpected request: ${path}`);
    }
  });

  const registered = await userModule.user.register({
    app: "user-module-demo",
    email: session.user.email,
    password: "demo123",
    displayName: session.user.displayName
  });
  const refreshed = await userModule.user.refresh();

  assert.deepEqual(registered, session);
  assert.deepEqual(refreshed, session);
  assert.equal(userModule.user.getToken(), session.token);
  assert.deepEqual(userModule.sessionStore.read(), session);
  assert.equal(requests[0]?.url.endsWith("/users/register"), true);
  assert.equal(requests[1]?.url.endsWith("/users/me"), true);
  assert.equal(requests[1]?.headers.get("authorization"), `Bearer ${session.token}`);

  userModule.user.logout();

  assert.equal(userModule.user.getToken(), undefined);
  assert.equal(userModule.sessionStore.read(), null);
});

test("session facts expose stable labels for the me panel", () => {
  assert.deepEqual(describeSessionFacts(null), [
    { label: "Status", value: "Signed out" },
    { label: "Current user", value: "None" },
    { label: "Tenant", value: "No active tenant" },
    { label: "Roles", value: "No active roles" }
  ]);

  assert.deepEqual(describeSessionFacts(demoSession()), [
    { label: "Status", value: "Signed in" },
    { label: "Current user", value: "Demo Person" },
    { label: "Tenant", value: "tenant_local" },
    { label: "Roles", value: "member" }
  ]);
});

test("session helpers expose developer-facing envelope details", () => {
  assert.deepEqual(describeSessionEnvelope(null), [
    { label: "App scope", value: "Awaiting login" },
    { label: "UID", value: "Not resolved yet" },
    { label: "Email", value: "No active session" },
    { label: "Token", value: "No bearer token" }
  ]);

  assert.deepEqual(describeSessionEnvelope(demoSession()), [
    { label: "App scope", value: "user-module-demo" },
    { label: "UID", value: "u_4001" },
    { label: "Email", value: "person@example.com" },
    { label: "Token", value: "token_us..._demo" }
  ]);
});

test("verification checklist reflects signed-out and signed-in progress", () => {
  assert.deepEqual(describeVerificationChecklist(null), [
    {
      label: "Login with seeded account",
      detail: "Use member@user.demo / demo123 to mint the first bearer token.",
      status: "active"
    },
    {
      label: "Refresh the session",
      detail: "Run db.user.me() after login and confirm the same user comes back.",
      status: "idle"
    },
    {
      label: "Register a fresh member",
      detail: "Create a new account and confirm it immediately becomes the active session.",
      status: "idle"
    },
    {
      label: "Logout cleanly",
      detail: "Revoke the browser session and confirm the snapshot returns to signed out.",
      status: "idle"
    }
  ]);

  assert.deepEqual(describeVerificationChecklist(demoSession()), [
    {
      label: "Login with seeded account",
      detail: "Use member@user.demo / demo123 to mint the first bearer token.",
      status: "ready"
    },
    {
      label: "Refresh the session",
      detail: "Run db.user.me() after login and confirm the same user comes back.",
      status: "active"
    },
    {
      label: "Register a fresh member",
      detail: "Create a new account and confirm it immediately becomes the active session.",
      status: "active"
    },
    {
      label: "Logout cleanly",
      detail: "Revoke the browser session and confirm the snapshot returns to signed out.",
      status: "active"
    }
  ]);
});
