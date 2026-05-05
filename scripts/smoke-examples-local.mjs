import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { resolveExampleStackUrls } from "./lib/example-stack.mjs";

const rootDir = process.cwd();
const authTokens = new Map();
const {
  gatewayUrl,
  ros2BackendUrl,
  flashSaleUrl,
  iotUrl,
  ros2Url,
  userModuleDemoUrl,
} = resolveExampleStackUrls();

const checks = [
  () => assertJson(`${gatewayUrl}/health`, { ok: true }, "gateway health"),
  () => assertRos2BackendHealth(),
  () => assertTopology(),
  () => assertAuthFlow("flash-sale", "buyer@flash-sale.demo", "demo123"),
  () => assertAuthFlow("iot-realtime", "operator@iot.demo", "demo123"),
  () => assertAuthFlow("ros2-bridge", "operator@ros2.demo", "demo123"),
  () => assertAuthFlow("user-module-demo", "member@user.demo", "demo123"),
  () => assertUserAliasFlow("flash-sale", "buyer@flash-sale.demo", "demo123"),
  () => assertUserAliasFlow("iot-realtime", "operator@iot.demo", "demo123"),
  () => assertUserAliasFlow("ros2-bridge", "operator@ros2.demo", "demo123"),
  () => assertUserAliasFlow("user-module-demo", "member@user.demo", "demo123"),
  () => assertStatus(flashSaleUrl, 200, "flash-sale app"),
  () => assertStatus(iotUrl, 200, "iot-realtime app"),
  () => assertStatus(ros2Url, 200, "ros2-bridge app"),
  () => assertStatus(userModuleDemoUrl, 200, "user-module-demo app"),
  () =>
    assertRoute(
      "flash-sale route",
      "flash-sale",
      "apps/examples/flash-sale/gateway/request.orders-read.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.route?.cluster === "flashsale.orders-sql" &&
        payload.data?.route?.service === "bladb-module-orders",
    ),
  () =>
    assertExecute(
      "flash-sale execute",
      "flash-sale",
      "apps/examples/flash-sale/gateway/request.orders-read.json",
      (payload) => payload.ok === true && Array.isArray(payload.data) && payload.data.length >= 1,
    ),
  () =>
    assertRoute(
      "iot route",
      "iot-realtime",
      "apps/examples/iot-realtime/gateway/request.reboot.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.route?.cluster === "iot.commands-mqtt" &&
        payload.data?.route?.service === "bladb-module-iot-mqtt",
    ),
  () =>
    assertExecute(
      "iot execute",
      "iot-realtime",
      "apps/examples/iot-realtime/gateway/request.reboot.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.published === true &&
        payload.data?.topic === "tenant/tenant_a/devices/device-001/commands",
    ),
  () =>
    assertRoute(
      "ros2 route",
      "ros2-bridge",
      "apps/examples/ros2-bridge/gateway/request.publish.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.route?.cluster === "ros2.bridge-mqtt" &&
        payload.data?.route?.service === "bladb-module-ros2-bridge",
    ),
  () =>
    assertExecute(
      "ros2 execute",
      "ros2-bridge",
      "apps/examples/ros2-bridge/gateway/request.publish.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.published === true &&
        payload.data?.fullTopic === "tenant/tenant_robotics/robots/robot-001/ros2/cmd_vel",
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

async function assertRos2BackendHealth() {
  const response = await fetch(`${ros2BackendUrl}/health`);
  const payload = await response.json();
  if (!response.ok || payload.ok !== true || payload.service !== "ros2-backend") {
    throw new Error(`ros2 backend health returned unexpected payload: ${JSON.stringify(payload)}`);
  }

  console.log("ros2 backend health: ok");
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
  authTokens.set(app, token);
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

  if (app === "ros2-bridge") {
    await assertRos2BridgeFlow(token);
  }

  console.log(`${app} auth: ok`);
}

async function assertUserAliasFlow(app, email, password) {
  const loginResponse = await fetch(`${gatewayUrl}/users/login`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify({ app, email, password }),
  });
  const loginPayload = await loginResponse.json();
  if (!loginResponse.ok || !loginPayload.data?.token) {
    throw new Error(`${app} users login failed: ${JSON.stringify(loginPayload)}`);
  }

  const token = loginPayload.data.token;
  const meResponse = await fetch(`${gatewayUrl}/users/me`, {
    headers: {
      authorization: `Bearer ${token}`,
    },
  });
  const mePayload = await meResponse.json();
  if (!meResponse.ok || mePayload.data?.user?.email !== email) {
    throw new Error(`${app} users me failed: ${JSON.stringify(mePayload)}`);
  }

  console.log(`${app} users alias: ok`);
}

async function assertExecute(label, app, relativeRequestPath, predicate) {
  const requestPath = path.join(rootDir, relativeRequestPath);
  const body = await readFile(requestPath, "utf8");
  const token = authTokens.get(app);
  const response = await fetch(`${gatewayUrl}/execute`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      ...(token ? { authorization: `Bearer ${token}` } : {}),
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

async function assertRoute(label, app, relativeRequestPath, predicate) {
  const requestPath = path.join(rootDir, relativeRequestPath);
  const body = await readFile(requestPath, "utf8");
  const token = authTokens.get(app);
  const response = await fetch(`${gatewayUrl}/route`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      ...(token ? { authorization: `Bearer ${token}` } : {}),
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
  const streamPromise = readFirstSseEvent(
    `${gatewayUrl}/apps/iot-realtime/commands/device-001/stream`,
    token,
    "mqtt-message",
  );
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

  const streamEvent = await streamPromise;
  if (
    streamEvent.deviceId !== "device-001" ||
    streamEvent.action !== "reboot" ||
    streamEvent.topic !== "tenant/tenant_a/devices/device-001/commands"
  ) {
    throw new Error(`iot command stream failed: ${JSON.stringify(streamEvent)}`);
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

  console.log("iot command stream: ok");
  console.log("iot command history: ok");
}

async function assertRos2BridgeFlow(token) {
  const streamPromise = readFirstSseEvent(
    `${gatewayUrl}/apps/ros2-bridge/messages/cmd_vel/stream`,
    token,
    "ros2-message",
  );
  const publishResponse = await fetch(`${gatewayUrl}/apps/ros2-bridge/messages`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      robotId: "robot-001",
      topicName: "cmd_vel",
      messageType: "geometry_msgs/msg/Twist",
      payload: {
        linear: { x: 0.4, y: 0, z: 0 },
        angular: { x: 0, y: 0, z: 0.15 }
      }
    }),
  });
  const publishPayload = await publishResponse.json();
  if (!publishResponse.ok || publishPayload.data?.published !== true) {
    throw new Error(`ros2 publish failed: ${JSON.stringify(publishPayload)}`);
  }

  const streamEvent = await streamPromise;
  if (
    streamEvent.topicName !== "cmd_vel" ||
    streamEvent.robotId !== "robot-001" ||
    streamEvent.messageType !== "geometry_msgs/msg/Twist"
  ) {
    throw new Error(`ros2 stream failed: ${JSON.stringify(streamEvent)}`);
  }

  const latestResponse = await fetch(`${gatewayUrl}/apps/ros2-bridge/messages/cmd_vel/latest`, {
    headers: {
      authorization: `Bearer ${token}`,
    },
  });
  const latestPayload = await latestResponse.json();
  if (!latestResponse.ok || latestPayload.data?.topicName !== "cmd_vel") {
    throw new Error(`ros2 latest failed: ${JSON.stringify(latestPayload)}`);
  }

  const historyResponse = await fetch(`${gatewayUrl}/apps/ros2-bridge/messages/cmd_vel`, {
    headers: {
      authorization: `Bearer ${token}`,
    },
  });
  const historyPayload = await historyResponse.json();
  if (!historyResponse.ok || !Array.isArray(historyPayload.data) || historyPayload.data.length < 1) {
    throw new Error(`ros2 history failed: ${JSON.stringify(historyPayload)}`);
  }

  console.log("ros2 stream: ok");
  console.log("ros2 bridge flow: ok");
}

async function readFirstSseEvent(url, token, expectedEvent) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(new Error(`timed out waiting for ${expectedEvent}`)), 8000);
  try {
    const response = await fetch(url, {
      headers: {
        authorization: `Bearer ${token}`,
      },
      signal: controller.signal,
    });

    if (!response.ok || !response.body) {
      throw new Error(`stream request failed with status ${response.status}`);
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffered = "";

    while (true) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }

      buffered += decoder.decode(value, { stream: true });
      let boundary = buffered.indexOf("\n\n");
      while (boundary !== -1) {
        const frame = buffered.slice(0, boundary).trim();
        buffered = buffered.slice(boundary + 2);

        if (!frame || frame.startsWith(":")) {
          boundary = buffered.indexOf("\n\n");
          continue;
        }

        const eventName = frame
          .split("\n")
          .find((line) => line.startsWith("event: "))
          ?.slice(7);
        const dataLines = frame
          .split("\n")
          .filter((line) => line.startsWith("data: "))
          .map((line) => line.slice(6));

        if (eventName === expectedEvent && dataLines.length > 0) {
          await reader.cancel();
          return JSON.parse(dataLines.join("\n"));
        }

        boundary = buffered.indexOf("\n\n");
      }
    }

    throw new Error(`stream ended before receiving ${expectedEvent}`);
  } finally {
    clearTimeout(timeout);
    controller.abort();
  }
}
