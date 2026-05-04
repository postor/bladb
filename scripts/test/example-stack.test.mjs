import assert from "node:assert/strict";
import test from "node:test";
import {
  parseDockerComposePort,
  resolveExampleStackUrls,
} from "../lib/example-stack.mjs";

test("resolveExampleStackUrls falls back to local defaults", () => {
  assert.deepEqual(resolveExampleStackUrls({}), {
    gatewayUrl: "http://127.0.0.1:8787",
    flashSaleUrl: "http://127.0.0.1:4173",
    iotUrl: "http://127.0.0.1:4174",
  });
});

test("resolveExampleStackUrls honors explicit environment overrides", () => {
  assert.deepEqual(
    resolveExampleStackUrls({
      BLADB_GATEWAY_URL: "http://127.0.0.1:50001",
      BLADB_FLASH_SALE_URL: "http://127.0.0.1:50002",
      BLADB_IOT_URL: "http://127.0.0.1:50003",
    }),
    {
      gatewayUrl: "http://127.0.0.1:50001",
      flashSaleUrl: "http://127.0.0.1:50002",
      iotUrl: "http://127.0.0.1:50003",
    },
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
