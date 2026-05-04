import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { resolveExampleStackUrls } from "./lib/example-stack.mjs";

const rootDir = process.cwd();
const {
  gatewayUrl,
  flashSaleUrl,
  iotUrl,
} = resolveExampleStackUrls();

const checks = [
  () => assertJson(`${gatewayUrl}/health`, { ok: true }, "gateway health"),
  () => assertTopology(),
  () => assertAuthFlow("flash-sale", "buyer@flash-sale.demo", "demo123"),
  () => assertAuthFlow("iot-realtime", "operator@iot.demo", "demo123"),
  () => assertStatus(flashSaleUrl, 200, "flash-sale app"),
  () => assertStatus(iotUrl, 200, "iot-realtime app"),
  () =>
    assertRoute(
      "flash-sale route",
      "apps/examples/flash-sale/gateway/request.orders-read.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.route?.cluster === "flashsale.orders-sql" &&
        payload.data?.route?.service === "bladb-module-orders",
    ),
  () =>
    assertExecute(
      "flash-sale execute",
      "apps/examples/flash-sale/gateway/request.orders-read.json",
      (payload) => payload.ok === true && Array.isArray(payload.data) && payload.data.length >= 1,
    ),
  () =>
    assertRoute(
      "iot route",
      "apps/examples/iot-realtime/gateway/request.reboot.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.route?.cluster === "iot.commands-mqtt" &&
        payload.data?.route?.service === "bladb-module-iot-mqtt",
    ),
  () =>
    assertExecute(
      "iot execute",
      "apps/examples/iot-realtime/gateway/request.reboot.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.published === true &&
        payload.data?.topic === "tenant/tenant_a/devices/device-001/commands",
    ),
];

try {
  for (const check of checks) {
    await check();
  }

  console.log("Example stack smoke test passed.");
} catch (error) {
  console.error(error.message);
  process.exit(1);
}

async function assertStatus(url, expectedStatus, label) {
  const response = await fetch(url);
  if (response.status !== expectedStatus) {
    throw new Error(`${label} returned ${response.status}, expected ${expectedStatus}`);
  }

  console.log(`${label}: ok`);
}

async function assertJson(url, expected, label) {
  const response = await fetch(url);
  const payload = await response.json();
  if (JSON.stringify(payload) !== JSON.stringify(expected)) {
    throw new Error(`${label} returned unexpected payload: ${JSON.stringify(payload)}`);
  }

  console.log(`${label}: ok`);
}

async function assertTopology() {
  const response = await fetch(`${gatewayUrl}/topology`);
  const payload = await response.json();
  if (
    payload.ok !== true ||
    !Array.isArray(payload.data) ||
    payload.data.length < 2 ||
    payload.data[0]?.clusters?.length < 1
  ) {
    throw new Error(`gateway topology returned unexpected payload: ${JSON.stringify(payload)}`);
  }

  console.log("gateway topology: ok");
}

async function assertAuthFlow(app, email, password) {
  const loginResponse = await fetch(`${gatewayUrl}/auth/login`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify({ app, email, password }),
  });
  const loginPayload = await loginResponse.json();
  if (!loginResponse.ok || !loginPayload.data?.token) {
    throw new Error(`${app} auth login failed: ${JSON.stringify(loginPayload)}`);
  }

  const token = loginPayload.data.token;
  const meResponse = await fetch(`${gatewayUrl}/auth/me`, {
    headers: {
      authorization: `Bearer ${token}`,
    },
  });
  const mePayload = await meResponse.json();
  if (!meResponse.ok || mePayload.data?.user?.email !== email) {
    throw new Error(`${app} auth me failed: ${JSON.stringify(mePayload)}`);
  }

  if (app === "flash-sale") {
    await assertQueueFlow(token);
  }

  if (app === "iot-realtime") {
    await assertIotCommandHistory(token);
  }

  console.log(`${app} auth: ok`);
}

async function assertExecute(label, relativeRequestPath, predicate) {
  const requestPath = path.join(rootDir, relativeRequestPath);
  const body = await readFile(requestPath, "utf8");
  const response = await fetch(`${gatewayUrl}/execute`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body,
  });
  const payload = await response.json();

  if (!predicate(payload)) {
    throw new Error(`${label} returned unexpected payload: ${JSON.stringify(payload)}`);
  }

  console.log(`${label}: ok`);
}

async function assertQueueFlow(token) {
  const summaryResponse = await fetch(`${gatewayUrl}/apps/flash-sale/summary`, {
    headers: {
      authorization: `Bearer ${token}`,
    },
  });
  const summaryPayload = await summaryResponse.json();
  if (!summaryResponse.ok || summaryPayload.data?.item?.sku !== "camera-pro") {
    throw new Error(`flash-sale summary failed: ${JSON.stringify(summaryPayload)}`);
  }

  const enqueueResponse = await fetch(`${gatewayUrl}/apps/flash-sale/queue`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      sku: "camera-pro",
      quantity: 1,
    }),
  });
  const enqueuePayload = await enqueueResponse.json();
  const ticketId = enqueuePayload.data?.ticketId;
  if (!enqueueResponse.ok || !ticketId) {
    throw new Error(`flash-sale queue enqueue failed: ${JSON.stringify(enqueuePayload)}`);
  }

  const deadline = Date.now() + 8000;
  while (Date.now() < deadline) {
    const statusResponse = await fetch(`${gatewayUrl}/apps/flash-sale/queue/${ticketId}`, {
      headers: {
        authorization: `Bearer ${token}`,
      },
    });
    const statusPayload = await statusResponse.json();
    const status = statusPayload.data?.status;
    if (!statusResponse.ok) {
      throw new Error(`flash-sale queue status failed: ${JSON.stringify(statusPayload)}`);
    }

    if (status === "completed" || status === "failed") {
      console.log("flash-sale queue: ok");
      return;
    }

    await new Promise((resolve) => setTimeout(resolve, 500));
  }

  throw new Error("flash-sale queue did not settle before timeout");
}

async function assertRoute(label, relativeRequestPath, predicate) {
  const requestPath = path.join(rootDir, relativeRequestPath);
  const body = await readFile(requestPath, "utf8");
  const response = await fetch(`${gatewayUrl}/route`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body,
  });
  const payload = await response.json();

  if (!predicate(payload)) {
    throw new Error(`${label} returned unexpected payload: ${JSON.stringify(payload)}`);
  }

  console.log(`${label}: ok`);
}

async function assertIotCommandHistory(token) {
  const publishResponse = await fetch(`${gatewayUrl}/apps/iot-realtime/commands`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      deviceId: "device-001",
      action: "reboot",
    }),
  });
  const publishPayload = await publishResponse.json();
  if (!publishResponse.ok || publishPayload.data?.published !== true) {
    throw new Error(`iot command publish failed: ${JSON.stringify(publishPayload)}`);
  }

  const response = await fetch(`${gatewayUrl}/apps/iot-realtime/commands`, {
    headers: {
      authorization: `Bearer ${token}`,
    },
  });
  const payload = await response.json();
  if (
    !response.ok ||
    !Array.isArray(payload.data) ||
    payload.data.length < 1 ||
    payload.data[0]?.deviceId !== "device-001"
  ) {
    throw new Error(`iot command history failed: ${JSON.stringify(payload)}`);
  }
}

