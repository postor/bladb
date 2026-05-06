import assert from "node:assert/strict";
import { mkdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import {
  EXAMPLE_STACK_STATE_PATH,
  clearExampleStackState,
  exampleStackUrlEnv,
  resolveExampleStackPorts,
  parseDockerComposePort,
  resolveExampleStackUrls,
} from "../lib/example-stack.mjs";

await clearExampleStackState();
const tmpDir = path.dirname(EXAMPLE_STACK_STATE_PATH);

test("resolveExampleStackUrls falls back to local defaults", () => {
  assert.deepEqual(resolveExampleStackUrls({}), {
    gatewayUrl: "http://127.0.0.1:8787",
    ros2BackendUrl: "http://127.0.0.1:8080",
    portalUrl: "http://127.0.0.1:4172",
    flashSaleUrl: "http://127.0.0.1:4173",
    blogUrl: "http://127.0.0.1:4174",
    iotUrl: "http://127.0.0.1:4175",
    ros2Url: "http://127.0.0.1:4176",
    userModuleDemoUrl: "http://127.0.0.1:4177",
  });
});

test("resolveExampleStackUrls prefers persisted stack ports when env overrides are absent", async () => {
  await mkdir(".tmp", { recursive: true });
  await writeFile(
    EXAMPLE_STACK_STATE_PATH,
    JSON.stringify({
      source: "test",
      ports: {
        gateway: 49001,
        ros2Backend: 49002,
        portal: 49003,
        flashSale: 49004,
        blog: 49005,
        iot: 49006,
        ros2: 49007,
        userModuleDemo: 49008,
      },
    }),
    "utf8",
  );

  assert.deepEqual(resolveExampleStackUrls({}), {
    gatewayUrl: "http://127.0.0.1:49001",
    ros2BackendUrl: "http://127.0.0.1:49002",
    portalUrl: "http://127.0.0.1:49003",
    flashSaleUrl: "http://127.0.0.1:49004",
    blogUrl: "http://127.0.0.1:49005",
    iotUrl: "http://127.0.0.1:49006",
    ros2Url: "http://127.0.0.1:49007",
    userModuleDemoUrl: "http://127.0.0.1:49008",
  });

  await clearExampleStackState();
});

test("resolveExampleStackUrls honors explicit URL environment overrides", () => {
  assert.deepEqual(
    resolveExampleStackUrls({
      BLADB_GATEWAY_URL: "http://127.0.0.1:50001",
      BLADB_ROS2_BACKEND_URL: "http://127.0.0.1:50002",
      BLADB_EXAMPLES_PORTAL_URL: "http://127.0.0.1:50003",
      BLADB_FLASH_SALE_URL: "http://127.0.0.1:50004",
      BLADB_BLOG_URL: "http://127.0.0.1:50005",
      BLADB_IOT_URL: "http://127.0.0.1:50006",
      BLADB_ROS2_URL: "http://127.0.0.1:50007",
      BLADB_USER_MODULE_DEMO_URL: "http://127.0.0.1:50008",
    }),
    {
      gatewayUrl: "http://127.0.0.1:50001",
      ros2BackendUrl: "http://127.0.0.1:50002",
      portalUrl: "http://127.0.0.1:50003",
      flashSaleUrl: "http://127.0.0.1:50004",
      blogUrl: "http://127.0.0.1:50005",
      iotUrl: "http://127.0.0.1:50006",
      ros2Url: "http://127.0.0.1:50007",
      userModuleDemoUrl: "http://127.0.0.1:50008",
    },
  );
});

test("resolveExampleStackUrls derives URLs from explicit port environment overrides", () => {
  assert.deepEqual(
    resolveExampleStackUrls({
      BLADB_GATEWAY_PORT: "50011",
      BLADB_ROS2_BACKEND_PORT: "50012",
      BLADB_EXAMPLES_PORTAL_PORT: "50013",
      BLADB_FLASH_SALE_URL: "http://127.0.0.1:50002",
      BLADB_BLOG_PORT: "50014",
      BLADB_IOT_PORT: "50015",
      BLADB_ROS2_PORT: "50016",
      BLADB_USER_MODULE_DEMO_PORT: "50017",
    }),
    {
      gatewayUrl: "http://127.0.0.1:50011",
      ros2BackendUrl: "http://127.0.0.1:50012",
      portalUrl: "http://127.0.0.1:50013",
      flashSaleUrl: "http://127.0.0.1:50002",
      blogUrl: "http://127.0.0.1:50014",
      iotUrl: "http://127.0.0.1:50015",
      ros2Url: "http://127.0.0.1:50016",
      userModuleDemoUrl: "http://127.0.0.1:50017",
    },
  );
});

test("exampleStackUrlEnv exposes compose-friendly URL variables", () => {
  assert.deepEqual(
    exampleStackUrlEnv({
      gatewayUrl: "http://127.0.0.1:50011",
      ros2BackendUrl: "http://127.0.0.1:50012",
      portalUrl: "http://127.0.0.1:50013",
      flashSaleUrl: "http://127.0.0.1:50014",
      blogUrl: "http://127.0.0.1:50015",
      iotUrl: "http://127.0.0.1:50016",
      ros2Url: "http://127.0.0.1:50017",
      userModuleDemoUrl: "http://127.0.0.1:50018",
    }),
    {
      BLADB_GATEWAY_URL: "http://127.0.0.1:50011",
      BLADB_ROS2_BACKEND_URL: "http://127.0.0.1:50012",
      BLADB_EXAMPLES_PORTAL_URL: "http://127.0.0.1:50013",
      BLADB_FLASH_SALE_URL: "http://127.0.0.1:50014",
      BLADB_BLOG_URL: "http://127.0.0.1:50015",
      BLADB_IOT_URL: "http://127.0.0.1:50016",
      BLADB_ROS2_URL: "http://127.0.0.1:50017",
      BLADB_USER_MODULE_DEMO_URL: "http://127.0.0.1:50018",
    },
  );
});

test("resolveExampleStackPorts auto-advances default ports when they are already busy", async () => {
  const busyPorts = new Set([4172, 4174, 4175]);

  assert.deepEqual(
    await resolveExampleStackPorts({
      env: {},
      isPortBusy: async (port) => busyPorts.has(port),
    }),
    {
      gateway: 8787,
      ros2Backend: 8080,
      portal: 4173,
      flashSale: 4176,
      blog: 4177,
      iot: 4178,
      ros2: 4179,
      userModuleDemo: 4180,
    },
  );
});

test("resolveExampleStackPorts rejects an explicitly configured busy port", async () => {
  await assert.rejects(
    resolveExampleStackPorts({
      env: {
        BLADB_USER_MODULE_DEMO_PORT: "4177",
      },
      isPortBusy: async (port) => port === 4177,
    }),
    /BLADB_USER_MODULE_DEMO_PORT is already in use on 127\.0\.0\.1:4177/,
  );
});

test("resolveExampleStackPorts skips fetch-restricted browser ports when auto-assigning", async () => {
  const busyPorts = new Set();
  for (let port = 4172; port <= 4189; port += 1) {
    busyPorts.add(port);
  }

  assert.deepEqual(
    await resolveExampleStackPorts({
      env: {},
      isPortBusy: async (port) => busyPorts.has(port),
    }),
    {
      gateway: 8787,
      ros2Backend: 8080,
      portal: 4191,
      flashSale: 4192,
      blog: 4193,
      iot: 4194,
      ros2: 4195,
      userModuleDemo: 4196,
    },
  );
});

test("resolveExampleStackPorts rejects an explicitly configured fetch-restricted port", async () => {
  await assert.rejects(
    resolveExampleStackPorts({
      env: {
        BLADB_ROS2_PORT: "4190",
      },
      isPortBusy: async () => false,
    }),
    /BLADB_ROS2_PORT uses a restricted browser\/fetch port on 127\.0\.0\.1:4190/,
  );
});

test("parseDockerComposePort extracts host port from docker compose output", () => {
  assert.equal(
    parseDockerComposePort("0.0.0.0:49152"),
    "http://127.0.0.1:49152",
  );
  assert.equal(
    parseDockerComposePort(":::49153"),
    "http://127.0.0.1:49153",
  );
});

test.after(async () => {
  await clearExampleStackState();
  try {
    await rm(tmpDir, { recursive: true, force: false, maxRetries: 0 });
  } catch (error) {
    if (
      error?.code !== "ENOTEMPTY" &&
      error?.code !== "ENOENT" &&
      error?.code !== "EBUSY" &&
      error?.code !== "EPERM"
    ) {
      throw error;
    }
  }
});
