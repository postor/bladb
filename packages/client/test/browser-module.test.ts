import assert from "node:assert/strict";
import test from "node:test";
import {
  BladbError,
  appGet,
  appPost,
  appStream,
  createClient,
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

test("client exposes db.user commands over the official users transport", async () => {
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

  const requests: Array<{ url: string; headers: Headers }> = [];
  const client = createClient({
    baseUrl: "http://localhost:8787",
    fetcher: async (input, init) => {
      const url = String(input);
      const headers = new Headers(init?.headers);
      requests.push({ url, headers });

      const path = new URL(url).pathname;
      if (path === "/users/login") {
        return jsonResponse(200, session);
      }

      if (path === "/users/me") {
        return jsonResponse(200, session);
      }

      throw new Error(`Unexpected request: ${path}`);
    },
    getToken: () => session.token
  });

  const loggedIn = await client.user.login({
    app: "flash-sale",
    email: "buyer@flash-sale.demo",
    password: "demo123"
  });

  const current = await client.user.me();

  assert.deepEqual(loggedIn, session);
  assert.deepEqual(current, session);
  assert.equal(requests[0]?.url.endsWith("/users/login"), true);
  assert.equal(requests[1]?.url.endsWith("/users/me"), true);
  assert.equal(requests[1]?.headers.get("authorization"), `Bearer ${session.token}`);
});

test("client keeps db.auth on the legacy auth transport for compatibility", async () => {
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

  const requests: Array<{ url: string; headers: Headers }> = [];
  const client = createClient({
    baseUrl: "http://localhost:8787",
    fetcher: async (input, init) => {
      const url = String(input);
      const headers = new Headers(init?.headers);
      requests.push({ url, headers });

      const path = new URL(url).pathname;
      if (path === "/auth/login") {
        return jsonResponse(200, session);
      }

      if (path === "/auth/me") {
        return jsonResponse(200, session);
      }

      throw new Error(`Unexpected request: ${path}`);
    },
    getToken: () => session.token
  });

  const loggedIn = await client.auth.login({
    app: "flash-sale",
    email: "buyer@flash-sale.demo",
    password: "demo123"
  });

  const current = await client.auth.me();

  assert.deepEqual(loggedIn, session);
  assert.deepEqual(current, session);
  assert.equal(requests[0]?.url.endsWith("/auth/login"), true);
  assert.equal(requests[1]?.url.endsWith("/auth/me"), true);
  assert.equal(requests[1]?.headers.get("authorization"), `Bearer ${session.token}`);
});

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

test("browser app module exposes user as the preferred managed session module", async () => {
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

  const flashSaleModule = createBrowserAppModule({
    baseUrl: "http://localhost:8787",
    appName: "flash-sale",
    tokenKey: "flash-sale.token",
    sessionKey: "flash-sale.session",
    routes: {
      queueHistory: appGet<QueueTicket[]>("queue")
    },
    fetcher: async (input) => {
      const path = new URL(String(input)).pathname;
      if (path === "/users/login" || path === "/users/me") {
        return jsonResponse(200, session);
      }

      if (path === "/users/logout") {
        return jsonResponse(200, {
          loggedOut: true
        });
      }

      if (path === "/apps/flash-sale/queue") {
        return jsonResponse(200, []);
      }

      throw new Error(`Unexpected request: ${path}`);
    }
  });

  const loggedIn = await flashSaleModule.user.login({
    app: "flash-sale",
    email: "buyer@flash-sale.demo",
    password: "demo123"
  });

  assert.deepEqual(loggedIn, session);
  assert.equal(flashSaleModule.user.getToken(), session.token);
  assert.equal(flashSaleModule.auth.getToken(), session.token);
  assert.equal(flashSaleModule.db.user.getToken(), session.token);
  assert.equal(flashSaleModule.db.auth.getToken(), session.token);
  assert.deepEqual(flashSaleModule.user.read(), session);
  assert.deepEqual(flashSaleModule.db.user.read(), session);

  flashSaleModule.db.user.logout();
  await Promise.resolve();

  assert.equal(flashSaleModule.user.getToken(), undefined);
  assert.equal(flashSaleModule.auth.getToken(), undefined);
  assert.equal(flashSaleModule.db.user.getToken(), undefined);
  assert.equal(flashSaleModule.sessionStore.read(), null);
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

test("browser app module can restore an anonymous session through db.user.me without a local token", async () => {
  installWindow();

  const session: GatewaySession = {
    token: "anon_flash_sale_token",
    sessionKind: "anonymous",
    anonymous: true,
    user: {
      app: "flash-sale",
      uid: "anon_flash_sale_3001",
      tenantId: "tenant_flash",
      email: "anon+anon_flash_sale_3001@flash-sale.local",
      displayName: "Anonymous flash sale 3001",
      roles: ["buyer"],
      anonymous: true
    }
  };

  const requests: Array<{ url: string; headers: Headers }> = [];
  const flashSaleModule = createBrowserAppModule({
    baseUrl: "http://localhost:8787",
    appName: "flash-sale",
    tokenKey: "flash-sale.cookie.token",
    sessionKey: "flash-sale.cookie.session",
    routes: {
      queueHistory: appGet<QueueTicket[]>("queue")
    },
    fetcher: async (input, init) => {
      const url = String(input);
      const headers = new Headers(init?.headers);
      requests.push({ url, headers });

      const parsed = new URL(url);
      if (parsed.pathname === "/users/me") {
        return jsonResponse(200, session);
      }

      throw new Error(`Unexpected request: ${parsed.pathname}`);
    }
  });

  const restored = await flashSaleModule.user.me();

  assert.deepEqual(restored, session);
  assert.equal(requests[0]?.url.endsWith("/users/me?app=flash-sale"), true);
  assert.equal(requests[0]?.headers.get("authorization"), null);
  assert.deepEqual(flashSaleModule.user.read(), session);
});

test("browser app module allows required app routes to proceed in cookie session mode without a bearer token", async () => {
  installWindow();

  const requests: Array<{ url: string; headers: Headers }> = [];
  const flashSaleModule = createBrowserAppModule({
    baseUrl: "http://localhost:8787",
    appName: "flash-sale",
    tokenKey: "flash-sale.required-cookie.token",
    sessionKey: "flash-sale.required-cookie.session",
    routes: {
      queueHistory: appGet<QueueTicket[]>("queue")
    },
    fetcher: async (input, init) => {
      const url = String(input);
      const headers = new Headers(init?.headers);
      requests.push({ url, headers });
      return jsonResponse(200, []);
    }
  });

  const history = await flashSaleModule.api.queueHistory();

  assert.deepEqual(history, []);
  assert.equal(requests[0]?.url.endsWith("/apps/flash-sale/queue"), true);
  assert.equal(requests[0]?.headers.get("authorization"), null);
});

test("browser session store drops orphaned session payload when bearer token is missing", async () => {
  installWindow();

  const sessionStore = createBrowserSessionStore<GatewaySession>({
    tokenKey: "iot.orphan.token",
    sessionKey: "iot.orphan.session"
  });

  const orphanedSession: GatewaySession = {
    token: "token_iot_orphaned",
    user: {
      app: "iot-realtime",
      uid: "operator_1",
      tenantId: "tenant_iot",
      email: "operator@iot.demo",
      displayName: "Operator One",
      roles: ["operator"]
    }
  };

  window.localStorage.setItem("iot.orphan.session", JSON.stringify(orphanedSession));

  assert.equal(sessionStore.getToken(), undefined);
  assert.equal(sessionStore.read(), null);
  assert.equal(window.localStorage.getItem("iot.orphan.session"), null);
});

test("plain client does not call required app routes without a bearer token or cookie session context", async () => {
  let requestCount = 0;
  const client = createClient({
    baseUrl: "http://localhost:8787",
    appAuth: "required",
    fetcher: async () => {
      requestCount += 1;
      throw new Error("fetcher should not be called without a token");
    }
  });

  await assert.rejects(
    client.app("flash-sale").get("queue"),
    (error: unknown) =>
      error instanceof BladbError &&
      error.status === 401 &&
      error.code === "AUTH_EXPIRED" &&
      error.message === "missing bearer token"
  );

  assert.equal(requestCount, 0);
});

test("plain client does not call execute routes without a bearer token or cookie session context", async () => {
  let requestCount = 0;
  const client = createClient({
    baseUrl: "http://localhost:8787",
    executeAuth: "required",
    fetcher: async () => {
      requestCount += 1;
      throw new Error("fetcher should not be called without a token");
    }
  });

  await assert.rejects(
    client.mongo("devices").find({
      ownerUid: "u_1001"
    }),
    (error: unknown) =>
      error instanceof BladbError &&
      error.status === 401 &&
      error.code === "AUTH_EXPIRED" &&
      error.message === "missing bearer token"
  );

  assert.equal(requestCount, 0);
});

test("browser app module streams app events with bearer auth", async () => {
  installWindow();

  const session: GatewaySession = {
    token: "token_ros2",
    user: {
      app: "ros2-bridge",
      uid: "u_3001",
      tenantId: "tenant_robotics",
      email: "operator@ros2.demo",
      displayName: "Robot Operator",
      roles: ["operator"]
    }
  };

  const streamedChunks = [
    'event: ros2-message\ndata: {"topicName":"cmd_vel","robotId":"robot-001","messageType":"geometry_msgs/msg/Twist"}\n\n'
  ];

  const requests: Array<{ url: string; headers: Headers }> = [];

  const ros2Module = createBrowserAppModule({
    baseUrl: "http://localhost:8787",
    appName: "ros2-bridge",
    tokenKey: "ros2.token",
    sessionKey: "ros2.session",
    routes: {
      latestMessage: appGet<unknown>("messages/cmd_vel/latest")
    },
    fetcher: async (input, init) => {
      const url = String(input);
      const headers = new Headers(init?.headers);
      requests.push({ url, headers });

      const path = new URL(url).pathname;
      if (path === "/auth/login") {
        return jsonResponse(200, session);
      }

      if (path === "/apps/ros2-bridge/messages/cmd_vel/stream") {
        const encoder = new TextEncoder();
        const stream = new ReadableStream({
          start(controller) {
            for (const chunk of streamedChunks) {
              controller.enqueue(encoder.encode(chunk));
            }
            controller.close();
          }
        });

        return new Response(stream, {
          status: 200,
          headers: {
            "content-type": "text/event-stream"
          }
        });
      }

      throw new Error(`Unexpected request: ${path}`);
    }
  });

  await ros2Module.auth.login({
    app: "ros2-bridge",
    email: "operator@ros2.demo",
    password: "demo123"
  });

  const messages: Array<Record<string, unknown>> = [];
  await ros2Module.db.app("ros2-bridge").stream("messages/cmd_vel/stream", {
    onMessage(payload) {
      messages.push(payload as Record<string, unknown>);
    }
  });

  assert.equal(messages.length, 1);
  assert.equal(messages[0]?.topicName, "cmd_vel");
  const streamRequest = requests.find((request) =>
    request.url.endsWith("/apps/ros2-bridge/messages/cmd_vel/stream")
  );
  assert.ok(streamRequest);
  assert.equal(streamRequest.headers.get("authorization"), `Bearer ${session.token}`);
  assert.equal(streamRequest.headers.get("accept"), "text/event-stream");
});

test("browser app module ignores keepalive stream frames before the first event", async () => {
  installWindow();

  const session: GatewaySession = {
    token: "token_ros2_keepalive",
    user: {
      app: "ros2-bridge",
      uid: "u_3001",
      tenantId: "tenant_robotics",
      email: "operator@ros2.demo",
      displayName: "Robot Operator",
      roles: ["operator"]
    }
  };

  const streamedChunks = [
    ": connected\n\n",
    'event: ros2-message\ndata: {"topicName":"cmd_vel","robotId":"robot-001","messageType":"geometry_msgs/msg/Twist"}\n\n'
  ];

  const ros2Module = createBrowserAppModule({
    baseUrl: "http://localhost:8787",
    appName: "ros2-bridge",
    tokenKey: "ros2.keepalive.token",
    sessionKey: "ros2.keepalive.session",
    routes: {
      latestMessage: appGet<unknown>("messages/cmd_vel/latest")
    },
    fetcher: async (input) => {
      const path = new URL(String(input)).pathname;
      if (path === "/auth/login") {
        return jsonResponse(200, session);
      }

      if (path === "/apps/ros2-bridge/messages/cmd_vel/stream") {
        const encoder = new TextEncoder();
        const stream = new ReadableStream({
          start(controller) {
            for (const chunk of streamedChunks) {
              controller.enqueue(encoder.encode(chunk));
            }
            controller.close();
          }
        });

        return new Response(stream, {
          status: 200,
          headers: {
            "content-type": "text/event-stream"
          }
        });
      }

      throw new Error(`Unexpected request: ${path}`);
    }
  });

  await ros2Module.auth.login({
    app: "ros2-bridge",
    email: "operator@ros2.demo",
    password: "demo123"
  });

  const messages: Array<Record<string, unknown>> = [];
  await ros2Module.db.app("ros2-bridge").stream("messages/cmd_vel/stream", {
    onMessage(payload) {
      messages.push(payload as Record<string, unknown>);
    }
  });

  assert.equal(messages.length, 1);
  assert.equal(messages[0]?.topicName, "cmd_vel");
  assert.equal(messages[0]?.robotId, "robot-001");
});

test("browser app module stream notifies when the event stream is opened", async () => {
  installWindow();

  const session: GatewaySession = {
    token: "token_iot_open",
    user: {
      app: "iot-realtime",
      uid: "u_1001",
      tenantId: "tenant_a",
      email: "operator@iot.demo",
      displayName: "IoT Operator",
      roles: ["operator"]
    }
  };

  const iotModule = createBrowserAppModule({
    baseUrl: "http://localhost:8787",
    appName: "iot-realtime",
    tokenKey: "iot.open.token",
    sessionKey: "iot.open.session",
    routes: {
      commandEvents: appStream<[deviceId: string], Record<string, unknown>>(
        (deviceId) => `commands/${deviceId}/stream`
      )
    },
    fetcher: async (input) => {
      const path = new URL(String(input)).pathname;
      if (path === "/auth/login") {
        return jsonResponse(200, session);
      }

      if (path === "/apps/iot-realtime/commands/device-001/stream") {
        const encoder = new TextEncoder();
        const stream = new ReadableStream({
          start(controller) {
            controller.enqueue(encoder.encode(": connected\n\n"));
            controller.close();
          }
        });

        return new Response(stream, {
          status: 200,
          headers: {
            "content-type": "text/event-stream"
          }
        });
      }

      throw new Error(`Unexpected request: ${path}`);
    }
  });

  await iotModule.auth.login({
    app: "iot-realtime",
    email: "operator@iot.demo",
    password: "demo123"
  });

  let opened = 0;
  await iotModule.api.commandEvents("device-001", {
    onOpen() {
      opened += 1;
    },
    onMessage() {}
  });

  assert.equal(opened, 1);
});

test("browser app module exposes typed stream routes for iot mqtt events", async () => {
  installWindow();

  const session: GatewaySession = {
    token: "token_iot",
    user: {
      app: "iot-realtime",
      uid: "u_1001",
      tenantId: "tenant_a",
      email: "operator@iot.demo",
      displayName: "IoT Operator",
      roles: ["operator"]
    }
  };

  const streamedChunks = [
    'event: mqtt-message\ndata: {"deviceId":"device-001","action":"reboot","topic":"tenant/tenant_a/devices/device-001/commands","issuedBy":"u_1001"}\n\n'
  ];

  const requests: Array<{ url: string; headers: Headers }> = [];

  const iotModule = createBrowserAppModule({
    baseUrl: "http://localhost:8787",
    appName: "iot-realtime",
    tokenKey: "iot.token",
    sessionKey: "iot.session",
    routes: {
      commandEvents: appStream<[deviceId: string], Record<string, unknown>>(
        (deviceId) => `commands/${deviceId}/stream`
      )
    },
    fetcher: async (input, init) => {
      const url = String(input);
      const headers = new Headers(init?.headers);
      requests.push({ url, headers });

      const path = new URL(url).pathname;
      if (path === "/auth/login") {
        return jsonResponse(200, session);
      }

      if (path === "/apps/iot-realtime/commands/device-001/stream") {
        const encoder = new TextEncoder();
        const stream = new ReadableStream({
          start(controller) {
            for (const chunk of streamedChunks) {
              controller.enqueue(encoder.encode(chunk));
            }
            controller.close();
          }
        });

        return new Response(stream, {
          status: 200,
          headers: {
            "content-type": "text/event-stream"
          }
        });
      }

      throw new Error(`Unexpected request: ${path}`);
    }
  });

  await iotModule.auth.login({
    app: "iot-realtime",
    email: "operator@iot.demo",
    password: "demo123"
  });

  const messages: Array<Record<string, unknown>> = [];
  await iotModule.api.commandEvents("device-001", {
    onMessage(payload) {
      messages.push(payload);
    }
  });

  assert.equal(messages.length, 1);
  assert.equal(messages[0]?.deviceId, "device-001");

  const streamRequest = requests.find((request) =>
    request.url.endsWith("/apps/iot-realtime/commands/device-001/stream")
  );
  assert.ok(streamRequest);
  assert.equal(streamRequest.headers.get("authorization"), `Bearer ${session.token}`);
  assert.equal(streamRequest.headers.get("accept"), "text/event-stream");
});

test("client mqtt publish keeps native-like topic and meta params for iot commands", async () => {
  const requests: Array<Record<string, unknown>> = [];
  const client = createClient({
    baseUrl: "http://localhost:8787",
    fetcher: async (_input, init) => {
      requests.push(JSON.parse(String(init?.body ?? "{}")) as Record<string, unknown>);
      return jsonResponse(200, {
        published: true,
        commandId: "cmd_0001",
        deviceId: "device-001",
        topic: "tenant/tenant_a/devices/device-001/commands",
        action: "reboot",
        issuedBy: "u_1001",
        createdAt: "2026-05-05T13:00:00Z"
      });
    }
  });

  await client
    .withMeta({
      resource: "device.command",
      policy: "iot.device-command.publish",
      params: {
        deviceId: "device-001"
      }
    })
    .mqtt.publish(
      "tenant/tenant_a/devices/device-001/commands",
      {
        action: "reboot",
        issuedBy: "u_1001"
      }
    );

  assert.equal(requests.length, 1);
  assert.equal(requests[0]?.engine, "mqtt");
  assert.equal(requests[0]?.action, "publish");
  assert.equal(requests[0]?.kind, "stream");
  assert.equal(requests[0]?.topic, "tenant/tenant_a/devices/device-001/commands");
  assert.deepEqual(requests[0]?.payload, {
    action: "reboot",
    issuedBy: "u_1001"
  });
  assert.deepEqual(requests[0]?.meta, {
    resource: "device.command",
    policy: "iot.device-command.publish",
    params: {
      deviceId: "device-001"
    }
  });
});
