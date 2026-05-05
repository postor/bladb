import assert from "node:assert/strict";
import test from "node:test";
import {
  resolveExampleStackPorts,
  parseDockerComposePort,
  resolveExampleStackUrls,
} from "../lib/example-stack.mjs";

test("resolveExampleStackUrls falls back to local defaults", () => {
  assert.deepEqual(resolveExampleStackUrls({}), {
    gatewayUrl: "http://127.0.0.1:8787",
    ros2BackendUrl: "http://127.0.0.1:8080",
    flashSaleUrl: "http://127.0.0.1:4173",
    iotUrl: "http://127.0.0.1:4174",
    ros2Url: "http://127.0.0.1:4175",
    userModuleDemoUrl: "http://127.0.0.1:4176",
  });
});

test("resolveExampleStackUrls honors explicit URL environment overrides", () => {
  assert.deepEqual(
    resolveExampleStackUrls({
      BLADB_GATEWAY_URL: "http://127.0.0.1:50001",
      BLADB_ROS2_BACKEND_URL: "http://127.0.0.1:50002",
      BLADB_FLASH_SALE_URL: "http://127.0.0.1:50003",
      BLADB_IOT_URL: "http://127.0.0.1:50004",
      BLADB_ROS2_URL: "http://127.0.0.1:50005",
      BLADB_USER_MODULE_DEMO_URL: "http://127.0.0.1:50006",
    }),
    {
      gatewayUrl: "http://127.0.0.1:50001",
      ros2BackendUrl: "http://127.0.0.1:50002",
      flashSaleUrl: "http://127.0.0.1:50003",
      iotUrl: "http://127.0.0.1:50004",
      ros2Url: "http://127.0.0.1:50005",
      userModuleDemoUrl: "http://127.0.0.1:50006",
    },
  );
});

test("resolveExampleStackUrls derives URLs from explicit port environment overrides", () => {
  assert.deepEqual(
    resolveExampleStackUrls({
      BLADB_GATEWAY_PORT: "50011",
      BLADB_ROS2_BACKEND_PORT: "50012",
      BLADB_FLASH_SALE_URL: "http://127.0.0.1:50002",
      BLADB_IOT_PORT: "50014",
      BLADB_ROS2_PORT: "50015",
      BLADB_USER_MODULE_DEMO_PORT: "50016",
    }),
    {
      gatewayUrl: "http://127.0.0.1:50011",
      ros2BackendUrl: "http://127.0.0.1:50012",
      flashSaleUrl: "http://127.0.0.1:50002",
      iotUrl: "http://127.0.0.1:50014",
      ros2Url: "http://127.0.0.1:50015",
      userModuleDemoUrl: "http://127.0.0.1:50016",
    },
  );
});

test("resolveExampleStackPorts auto-advances default ports when they are already busy", async () => {
  const busyPorts = new Set([4174, 4175]);

  assert.deepEqual(
    await resolveExampleStackPorts({
      env: {},
      isPortBusy: async (port) => busyPorts.has(port),
    }),
    {
      gateway: 8787,
      ros2Backend: 8080,
      flashSale: 4173,
      iot: 4176,
      ros2: 4177,
      userModuleDemo: 4178,
    },
  );
});

test("resolveExampleStackPorts rejects an explicitly configured busy port", async () => {
  await assert.rejects(
    resolveExampleStackPorts({
      env: {
        BLADB_USER_MODULE_DEMO_PORT: "4176",
      },
      isPortBusy: async (port) => port === 4176,
    }),
    /BLADB_USER_MODULE_DEMO_PORT is already in use on 127\.0\.0\.1:4176/,
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
