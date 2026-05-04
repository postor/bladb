import assert from "node:assert/strict";
import test from "node:test";
import {
  BladbError,
  appGet,
  appPost,
  createBrowserAppModule,
  createBrowserAuthModule,
  createBrowserSessionStore,
  type GatewaySession
} from "../src/index.ts";

interface QueueTicket {
  ticketId: string;
  sku: string;
  quantity: number;
  status: "queued" | "processing" | "completed" | "failed";
  queuePosition: number | null;
  orderId: string | null;
  message: string;
  createdAt: string;
  updatedAt: string;
}

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

test("browser app module persists auth sessions and reuses the token for app routes", async () => {
  installWindow();

  const session: GatewaySession = {
    token: "token_flash_sale",
    user: {
      app: "flash-sale",
      uid: "buyer_1",
      tenantId: "tenant_flash",
      email: "buyer@flash-sale.demo",
      displayName: "Buyer One",
      roles: ["buyer"]
    }
  };

  const queueHistory: QueueTicket[] = [
    {
      ticketId: "ticket_01",
      sku: "camera-pro",
      quantity: 1,
      status: "completed",
      queuePosition: null,
      orderId: "order_01",
      message: "completed",
      createdAt: "2026-05-04T10:00:00.000Z",
      updatedAt: "2026-05-04T10:00:05.000Z"
    }
  ];

  const requests: Array<{ url: string; headers: Headers }> = [];

  const flashSaleModule = createBrowserAppModule({
    baseUrl: "http://localhost:8787",
    appName: "flash-sale",
    tokenKey: "flash-sale.token",
    sessionKey: "flash-sale.session",
    routes: {
      queuePurchase: appPost<{ sku: string; quantity: number }, QueueTicket>("queue"),
      queueHistory: appGet<QueueTicket[]>("queue")
    },
    fetcher: async (input, init) => {
      const url = String(input);
      const headers = new Headers(init?.headers);
      requests.push({ url, headers });

      const path = new URL(url).pathname;
      if (path === "/auth/login") {
        return jsonResponse(200, session);
      }

      if (path === "/apps/flash-sale/queue") {
        return jsonResponse(200, queueHistory);
      }

      throw new Error(`Unexpected request: ${path}`);
    }
  });

  await flashSaleModule.auth.login({
    app: "flash-sale",
    email: "buyer@flash-sale.demo",
    password: "demo123"
  });

  assert.deepEqual(flashSaleModule.sessionStore.read(), session);
  assert.equal(flashSaleModule.auth.getToken(), session.token);

  const history = await flashSaleModule.api.queueHistory();
  assert.deepEqual(history, queueHistory);

  const appRequest = requests.find((request) => request.url.endsWith("/apps/flash-sale/queue"));
  assert.ok(appRequest);
  assert.equal(appRequest.headers.get("authorization"), `Bearer ${session.token}`);
});

test("browser auth module clears stale sessions when refresh fails", async () => {
  installWindow();

  const sessionStore = createBrowserSessionStore<GatewaySession>({
    tokenKey: "iot.token",
    sessionKey: "iot.session"
  });

  const existingSession: GatewaySession = {
    token: "token_iot",
    user: {
      app: "iot-realtime",
      uid: "operator_1",
      tenantId: "tenant_iot",
      email: "operator@iot.demo",
      displayName: "Operator One",
      roles: ["operator"]
    }
  };

  sessionStore.persist(existingSession);

  const auth = createBrowserAuthModule(
    {
      login: async () => existingSession,
      register: async () => existingSession,
      me: async () => {
        throw new BladbError("Unauthorized", {
          status: 401
        });
      }
    },
    sessionStore
  );

  const refreshed = await auth.refresh();

  assert.equal(refreshed, null);
  assert.equal(auth.read(), null);
  assert.equal(auth.getToken(), undefined);
});
