import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { resolveExampleStackUrls } from "./lib/example-stack.mjs";

const rootDir = process.cwd();
const authTokens = new Map();
const {
  gatewayUrl,
  ros2BackendUrl,
  portalUrl,
  flashSaleUrl,
  blogUrl,
  iotUrl,
  ros2Url,
  userModuleDemoUrl,
} = resolveExampleStackUrls();

const checks = [
  { label: "gateway health", run: () => assertJson(`${gatewayUrl}/health`, { ok: true }, "gateway health") },
  { label: "ros2 backend health", run: () => assertRos2BackendHealth() },
  { label: "gateway topology", run: () => assertTopology() },
  { label: "examples-portal app", run: () => assertStatus(portalUrl, 200, "examples-portal app") },
  { label: "flash-sale app", run: () => assertStatus(flashSaleUrl, 200, "flash-sale app") },
  { label: "blog app", run: () => assertStatus(blogUrl, 200, "blog app") },
  { label: "iot-realtime app", run: () => assertStatus(iotUrl, 200, "iot-realtime app") },
  { label: "ros2-bridge app", run: () => assertStatus(ros2Url, 200, "ros2-bridge app") },
  { label: "user-module-demo app", run: () => assertStatus(userModuleDemoUrl, 200, "user-module-demo app") },
  {
    label: "examples-portal suite ui",
    run: () => assertPortalFlow(),
  },
  {
    label: "flash-sale anonymous ui",
    run: () => assertPageDoesNotContain(flashSaleUrl, /login|register/i, "flash-sale anonymous ui"),
  },
  {
    label: "iot anonymous ui",
    run: () => assertPageDoesNotContain(iotUrl, /login|register/i, "iot anonymous ui"),
  },
  {
    label: "ros2 anonymous ui",
    run: () => assertPageDoesNotContain(ros2Url, /login|register/i, "ros2 anonymous ui"),
  },
  {
    label: "blog public ui",
    run: () => assertBlogPublicFlow(),
  },
  {
    label: "blog anonymous identity flow",
    run: () => assertAnonymousBlogFlow(),
  },
  {
    label: "flash-sale auth",
    run: () => assertAuthFlow("flash-sale", "buyer@flash-sale.demo", "demo123"),
  },
  {
    label: "iot auth",
    run: () => assertAuthFlow("iot-realtime", "operator@iot.demo", "demo123"),
  },
  {
    label: "ros2 auth",
    run: () => assertAuthFlow("ros2-bridge", "operator@ros2.demo", "demo123"),
  },
  {
    label: "user-module-demo auth",
    run: () => assertAuthFlow("user-module-demo", "member@user.demo", "demo123"),
  },
  {
    label: "user-module-demo users alias",
    run: () => assertUserAliasFlow("user-module-demo", "member@user.demo", "demo123"),
  },
  { label: "blog auth", run: () => assertAuthFlow("blog", "editor@blog.demo", "demo123") },
  { label: "blog users alias", run: () => assertUserAliasFlow("blog", "editor@blog.demo", "demo123") },
  { label: "flash-sale anonymous flow", run: () => assertAnonymousFlashSaleFlow() },
  { label: "iot anonymous flow", run: () => assertAnonymousIotFlow() },
  { label: "ros2 anonymous flow", run: () => assertAnonymousRos2Flow() },
  { label: "blog mongo + user flow", run: () => assertBlogFlow() },
  {
    label: "flash-sale route",
    run: () =>
      assertRoute(
      "flash-sale route",
      "flash-sale",
      "apps/examples/flash-sale/gateway/request.orders-read.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.route?.cluster === "flashsale.orders-sql" &&
        payload.data?.route?.service === "bladb-module-orders",
    ),
  },
  {
    label: "flash-sale execute",
    run: () =>
      assertExecute(
      "flash-sale execute",
      "flash-sale",
      "apps/examples/flash-sale/gateway/request.orders-read.json",
      (payload) => payload.ok === true && Array.isArray(payload.data) && payload.data.length >= 1,
    ),
  },
  {
    label: "iot route",
    run: () =>
      assertRoute(
      "iot route",
      "iot-realtime",
      "apps/examples/iot-realtime/gateway/request.reboot.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.route?.cluster === "iot.commands-mqtt" &&
        payload.data?.route?.service === "bladb-module-iot-mqtt",
    ),
  },
  {
    label: "iot execute",
    run: () =>
      assertExecute(
      "iot execute",
      "iot-realtime",
      "apps/examples/iot-realtime/gateway/request.reboot.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.published === true &&
        payload.data?.topic === "tenant/tenant_a/devices/device-001/commands",
    ),
  },
  {
    label: "ros2 route",
    run: () =>
      assertRoute(
      "ros2 route",
      "ros2-bridge",
      "apps/examples/ros2-bridge/gateway/request.publish.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.route?.cluster === "ros2.bridge-mqtt" &&
        payload.data?.route?.service === "bladb-module-ros2-bridge",
    ),
  },
  {
    label: "ros2 execute",
    run: () =>
      assertExecute(
      "ros2 execute",
      "ros2-bridge",
      "apps/examples/ros2-bridge/gateway/request.publish.json",
      (payload) =>
        payload.ok === true &&
        payload.data?.published === true &&
        payload.data?.fullTopic === "tenant/tenant_robotics/robots/robot-001/ros2/cmd_vel",
    ),
  },
];

try {
  for (const check of checks) {
    try {
      await check.run();
    } catch (error) {
      throw new Error(`${check.label} failed: ${error.message}`);
    }
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

async function assertPageDoesNotContain(url, pattern, label) {
  const response = await fetch(url);
  const html = await response.text();
  if (pattern.test(html)) {
    throw new Error(`${label} still contains auth shell text`);
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
    payload.data.length < 3 ||
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
  const token = app ? authTokens.get(app) : undefined;
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

async function assertRoute(label, app, relativeRequestPath, predicate) {
  const requestPath = path.join(rootDir, relativeRequestPath);
  const body = await readFile(requestPath, "utf8");
  const token = app ? authTokens.get(app) : undefined;
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

async function assertAnonymousFlashSaleFlow() {
  const summaryResponse = await fetch(`${gatewayUrl}/apps/flash-sale/summary`);
  const summaryPayload = await summaryResponse.json();
  if (!summaryResponse.ok || summaryPayload.data?.item?.sku !== "camera-pro") {
    throw new Error(`anonymous flash-sale summary failed: ${JSON.stringify(summaryPayload)}`);
  }

  const sessionCookie = extractSessionCookie(summaryResponse, "anonymous flash-sale summary");
  await assertAnonymousMe("flash-sale", sessionCookie, summaryPayload.data?.identity?.uid);

  const enqueueResponse = await fetch(`${gatewayUrl}/apps/flash-sale/queue`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      cookie: sessionCookie,
    },
    body: JSON.stringify({
      sku: "camera-pro",
      quantity: 1,
    }),
  });
  const enqueuePayload = await enqueueResponse.json();
  const ticketId = enqueuePayload.data?.ticketId;
  if (!enqueueResponse.ok || !ticketId) {
    throw new Error(`anonymous flash-sale queue enqueue failed: ${JSON.stringify(enqueuePayload)}`);
  }

  const deadline = Date.now() + 8000;
  while (Date.now() < deadline) {
    const statusResponse = await fetch(`${gatewayUrl}/apps/flash-sale/queue/${ticketId}`, {
      headers: {
        cookie: sessionCookie,
      },
    });
    const statusPayload = await statusResponse.json();
    const status = statusPayload.data?.status;
    if (!statusResponse.ok) {
      throw new Error(`anonymous flash-sale queue status failed: ${JSON.stringify(statusPayload)}`);
    }

    if (status === "completed" || status === "failed") {
      if (statusPayload.data?.runtime?.queueCluster !== "flashsale.workflow-workers") {
        throw new Error(`anonymous flash-sale runtime metadata failed: ${JSON.stringify(statusPayload)}`);
      }
      console.log("flash-sale anonymous queue: ok");
      return;
    }

    await new Promise((resolve) => setTimeout(resolve, 500));
  }

  throw new Error("anonymous flash-sale queue did not settle before timeout");
}

async function assertAnonymousIotFlow() {
  const devicesResponse = await fetch(`${gatewayUrl}/apps/iot-realtime/devices`);
  const devicesPayload = await devicesResponse.json();
  if (!devicesResponse.ok || !Array.isArray(devicesPayload.data) || devicesPayload.data.length < 1) {
    throw new Error(`anonymous iot devices failed: ${JSON.stringify(devicesPayload)}`);
  }

  const sessionCookie = extractSessionCookie(devicesResponse, "anonymous iot devices");
  const deviceId = devicesPayload.data[0]?.id;
  await assertAnonymousMe("iot-realtime", sessionCookie);

  const streamPromise = readFirstSseEvent(
    `${gatewayUrl}/apps/iot-realtime/commands/${deviceId}/stream`,
    "mqtt-message",
    {
      cookie: sessionCookie,
    },
  );
  const publishResponse = await fetch(`${gatewayUrl}/apps/iot-realtime/commands`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      cookie: sessionCookie,
    },
    body: JSON.stringify({
      deviceId,
      action: "reboot",
    }),
  });
  const publishPayload = await publishResponse.json();
  if (!publishResponse.ok || publishPayload.data?.published !== true) {
    throw new Error(`anonymous iot command publish failed: ${JSON.stringify(publishPayload)}`);
  }

  const streamEvent = await streamPromise;
  if (
    streamEvent.deviceId !== deviceId ||
    streamEvent.action !== "reboot" ||
    streamEvent.topic !== `tenant/tenant_a/devices/${deviceId}/commands`
  ) {
    throw new Error(`anonymous iot command stream failed: ${JSON.stringify(streamEvent)}`);
  }

  const response = await fetch(`${gatewayUrl}/apps/iot-realtime/commands`, {
    headers: {
      cookie: sessionCookie,
    },
  });
  const payload = await response.json();
  if (!response.ok || !Array.isArray(payload.data) || payload.data[0]?.deviceId !== deviceId) {
    throw new Error(`anonymous iot command history failed: ${JSON.stringify(payload)}`);
  }

  console.log("iot anonymous flow: ok");
}

async function assertAnonymousRos2Flow() {
  const latestResponse = await fetch(`${gatewayUrl}/apps/ros2-bridge/messages/cmd_vel/latest`);
  const latestPayload = await latestResponse.json();
  if (!latestResponse.ok || latestPayload.data?.topicName !== "cmd_vel") {
    throw new Error(`anonymous ros2 latest failed: ${JSON.stringify(latestPayload)}`);
  }

  const sessionCookie = extractSessionCookie(latestResponse, "anonymous ros2 latest");
  await assertAnonymousMe("ros2-bridge", sessionCookie);

  const streamPromise = readFirstSseEvent(
    `${gatewayUrl}/apps/ros2-bridge/messages/cmd_vel/stream`,
    "ros2-message",
    {
      cookie: sessionCookie,
    },
  );
  const publishResponse = await fetch(`${gatewayUrl}/apps/ros2-bridge/messages`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      cookie: sessionCookie,
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
    throw new Error(`anonymous ros2 publish failed: ${JSON.stringify(publishPayload)}`);
  }

  const streamEvent = await streamPromise;
  if (
    streamEvent.topicName !== "cmd_vel" ||
    streamEvent.robotId !== "robot-001" ||
    streamEvent.messageType !== "geometry_msgs/msg/Twist"
  ) {
    throw new Error(`anonymous ros2 stream failed: ${JSON.stringify(streamEvent)}`);
  }

  const refreshedLatestResponse = await fetch(`${gatewayUrl}/apps/ros2-bridge/messages/cmd_vel/latest`, {
    headers: {
      cookie: sessionCookie,
    },
  });
  const refreshedLatestPayload = await refreshedLatestResponse.json();
  if (!refreshedLatestResponse.ok || refreshedLatestPayload.data?.topicName !== "cmd_vel") {
    throw new Error(`anonymous ros2 latest refresh failed: ${JSON.stringify(refreshedLatestPayload)}`);
  }

  console.log("ros2 anonymous flow: ok");
}

async function assertBlogFlow() {
  const publicListResponse = await fetch(`${gatewayUrl}/apps/blog/posts`);
  const publicListPayload = await publicListResponse.json();
  if (!publicListResponse.ok || !Array.isArray(publicListPayload.data) || publicListPayload.data.length < 1) {
    throw new Error(`blog public list failed: ${JSON.stringify(publicListPayload)}`);
  }

  const foreignSeedPost = publicListPayload.data.find((post) => post.authorUid !== "u_5001");
  if (!foreignSeedPost) {
    throw new Error(`blog public list is missing a second author's article: ${JSON.stringify(publicListPayload)}`);
  }

  const token = authTokens.get("blog");
  const uniqueSuffix = `${Date.now()}`;
  const createdTitle = `Smoke test post ${uniqueSuffix}`;
  const createdSlug = `smoke-test-post-${uniqueSuffix}`;
  const createResponse = await fetch(`${gatewayUrl}/execute`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      kind: "command",
      engine: "mongo",
      action: "insertOne",
      meta: {
        policy: "blog.posts.create"
      },
      collection: "posts",
      document: {
        tenantId: { $ctx: "tenantId", token: "TENANT_ID" },
        authorUid: { $ctx: "uid", token: "UID" },
        authorName: "Blog Editor",
        title: createdTitle,
        slug: createdSlug,
        summary: "Created during smoke validation.",
        body: "Verifying mongo + user integration.",
        published: true
      }
    }),
  });
  const createPayload = await createResponse.json();
  const createdPostId = createPayload.data?.id;
  if (!createResponse.ok || createPayload.data?.slug !== createdSlug || !createdPostId) {
    throw new Error(`blog create failed: ${JSON.stringify(createPayload)}`);
  }

  const mineResponse = await fetch(`${gatewayUrl}/execute`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      kind: "query",
      engine: "mongo",
      action: "find",
      meta: {
        policy: "blog.posts.list-mine"
      },
      collection: "posts",
      query: {
        tenantId: { $ctx: "tenantId", token: "TENANT_ID" },
        authorUid: { $ctx: "uid", token: "UID" }
      },
      options: {
        limit: 20
      }
    }),
  });
  const minePayload = await mineResponse.json();
  if (
    !mineResponse.ok ||
    !Array.isArray(minePayload.data) ||
    !minePayload.data.some((post) => post.slug === createdSlug)
  ) {
    throw new Error(`blog mine failed: ${JSON.stringify(minePayload)}`);
  }

  const updateResponse = await fetch(`${gatewayUrl}/apps/blog/me/posts/${createdPostId}`, {
    method: "PATCH",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      title: `${createdTitle} updated`,
      slug: `${createdSlug}-updated`,
      summary: "Updated during smoke validation.",
      body: "Updated body during smoke validation.",
      published: true,
    }),
  });
  const updatePayload = await updateResponse.json();
  if (!updateResponse.ok || updatePayload.data?.slug !== `${createdSlug}-updated`) {
    throw new Error(`blog update failed: ${JSON.stringify(updatePayload)}`);
  }

  const hackedUpdateResponse = await fetch(`${gatewayUrl}/execute`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      kind: "command",
      engine: "mongo",
      action: "updateOne",
      meta: {
        policy: "blog.posts.update-mine"
      },
      collection: "posts",
      query: {
        id: foreignSeedPost.id,
        tenantId: { $ctx: "tenantId", token: "TENANT_ID" },
        authorUid: { $ctx: "uid", token: "UID" }
      },
      document: {
        title: "Hacked title",
        slug: "hacked-title",
        summary: "This should fail",
        body: "This should fail",
        published: true
      }
    }),
  });
  const hackedUpdatePayload = await hackedUpdateResponse.json();
  if (hackedUpdateResponse.ok || !String(hackedUpdatePayload?.error?.message ?? hackedUpdatePayload?.message ?? "").includes("another author's article")) {
    throw new Error(`blog forged update should fail but returned: ${JSON.stringify(hackedUpdatePayload)}`);
  }

  const publishedResponse = await fetch(`${gatewayUrl}/apps/blog/posts`);
  const publishedPayload = await publishedResponse.json();
  if (
    !publishedResponse.ok ||
    !Array.isArray(publishedPayload.data) ||
    !publishedPayload.data.some((post) => post.slug === `${createdSlug}-updated`)
  ) {
    throw new Error(`blog published list refresh failed: ${JSON.stringify(publishedPayload)}`);
  }

  const deleteResponse = await fetch(`${gatewayUrl}/apps/blog/me/posts/${createdPostId}`, {
    method: "DELETE",
    headers: {
      authorization: `Bearer ${token}`,
    },
  });
  const deletePayload = await deleteResponse.json();
  if (!deleteResponse.ok || deletePayload.data?.deleted !== true) {
    throw new Error(`blog delete failed: ${JSON.stringify(deletePayload)}`);
  }

  const myPostsAfterDelete = await fetch(`${gatewayUrl}/apps/blog/me/posts`, {
    headers: {
      authorization: `Bearer ${token}`,
    },
  });
  const myPostsAfterDeletePayload = await myPostsAfterDelete.json();
  if (
    !myPostsAfterDelete.ok ||
    !Array.isArray(myPostsAfterDeletePayload.data) ||
    myPostsAfterDeletePayload.data.some((post) => post.id === createdPostId)
  ) {
    throw new Error(`blog delete verification failed: ${JSON.stringify(myPostsAfterDeletePayload)}`);
  }

  console.log("blog mongo + user flow: ok");
}

async function assertBlogPublicFlow() {
  const response = await fetch(`${gatewayUrl}/apps/blog/posts`);
  const payload = await response.json();
  if (!response.ok || !Array.isArray(payload.data) || payload.data.length < 1) {
    throw new Error(`blog public list failed: ${JSON.stringify(payload)}`);
  }

  const authors = new Set(payload.data.map((post) => post.authorUid));
  if (authors.size < 2) {
    throw new Error(`blog public plaza should show multiple authors: ${JSON.stringify(payload)}`);
  }

  console.log("blog public ui: ok");
}

async function assertAnonymousBlogFlow() {
  const publicListResponse = await fetch(`${gatewayUrl}/apps/blog/posts`);
  const publicListPayload = await publicListResponse.json();
  if (!publicListResponse.ok || !Array.isArray(publicListPayload.data) || publicListPayload.data.length < 1) {
    throw new Error(`anonymous blog public list failed: ${JSON.stringify(publicListPayload)}`);
  }

  const sessionCookie = extractSessionCookie(publicListResponse, "anonymous blog public list");
  const firstIdentity = await readAnonymousMe("blog", sessionCookie);

  const repeatedPublicListResponse = await fetch(`${gatewayUrl}/apps/blog/posts`, {
    headers: {
      cookie: sessionCookie,
    },
  });
  const repeatedPublicListPayload = await repeatedPublicListResponse.json();
  if (
    !repeatedPublicListResponse.ok ||
    !Array.isArray(repeatedPublicListPayload.data) ||
    repeatedPublicListPayload.data.length < 1
  ) {
    throw new Error(`anonymous blog repeated public list failed: ${JSON.stringify(repeatedPublicListPayload)}`);
  }

  const secondIdentity = await readAnonymousMe("blog", sessionCookie);
  if (firstIdentity.user.uid !== secondIdentity.user.uid) {
    throw new Error(
      `anonymous blog identity did not persist across public reads: ${JSON.stringify({
        firstUid: firstIdentity.user.uid,
        secondUid: secondIdentity.user.uid,
      })}`,
    );
  }

  console.log("blog anonymous identity flow: ok");
}

async function assertPortalFlow() {
  const response = await fetch(portalUrl);
  const html = await response.text();
  const markers = [
    "One entry point for every Bladb demo",
    "Flash Sale",
    "User Module Demo",
  ];

  if (containsAllMarkers(html, markers)) {
    console.log("examples-portal suite ui: ok");
    return;
  }

  const assetPaths = collectPortalAssetPaths(html);
  for (const assetPath of assetPaths) {
    try {
      const assetUrl = new URL(assetPath, portalUrl).toString();
      const assetResponse = await fetch(assetUrl);
      if (!assetResponse.ok) {
        continue;
      }

      const assetText = await assetResponse.text();
      if (containsAllMarkers(assetText, markers)) {
        console.log("examples-portal suite ui: ok");
        return;
      }
    } catch {
      // Ignore non-critical asset fetch failures and continue probing.
    }
  }

  throw new Error("portal page assets are missing suite content markers");
}

function collectPortalAssetPaths(html) {
  const matches = Array.from(
    html.matchAll(/<(?:script|link)[^>]+(?:src|href)=["']([^"']+)["']/g),
    (match) => match[1],
  );

  return [
    ...new Set([
      ...matches,
      "/src/App.tsx",
      "/src/main.tsx",
    ]),
  ];
}

function containsAllMarkers(text, markers) {
  return markers.every((marker) => text.includes(marker));
}

function extractSessionCookie(response, label) {
  const cookieHeader = response.headers.get("set-cookie");
  const sessionCookie = cookieHeader?.split(";").at(0);
  if (!sessionCookie) {
    throw new Error(`${label} did not return a session cookie`);
  }

  return sessionCookie;
}

async function assertAnonymousMe(app, sessionCookie, expectedUid = undefined) {
  const payload = await readAnonymousMe(app, sessionCookie);
  if (expectedUid && payload.user.uid !== expectedUid) {
    throw new Error(
      `anonymous ${app} me returned unexpected uid: ${JSON.stringify({
        expectedUid,
        actualUid: payload.user.uid,
      })}`,
    );
  }
}

async function readAnonymousMe(app, sessionCookie) {
  const meResponse = await fetch(`${gatewayUrl}/users/me?app=${app}`, {
    headers: {
      cookie: sessionCookie,
    },
  });
  const mePayload = await meResponse.json();
  if (
    !meResponse.ok ||
    mePayload.data?.user?.app !== app ||
    mePayload.data?.anonymous !== true
  ) {
    throw new Error(`anonymous ${app} me failed: ${JSON.stringify(mePayload)}`);
  }

  return mePayload.data;
}

async function readFirstSseEvent(url, expectedEvent, extraHeaders = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(new Error(`timed out waiting for ${expectedEvent}`)), 8000);
  try {
    const response = await fetch(url, {
      headers: {
        ...extraHeaders,
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

      buffered += decoder.decode(value, { stream: true }).replace(/\r/g, "");
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
