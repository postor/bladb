import assert from "node:assert/strict";
import test from "node:test";
import { resolveIotScreenState } from "./session-view.ts";

test("iot session view stays in restoring state until auth is ready", () => {
  assert.equal(
    resolveIotScreenState({
      ready: false,
      session: {
        token: "stale-token"
      }
    }),
    "restoring"
  );
});

test("iot session view shows login when auth is ready and session is missing", () => {
  assert.equal(
    resolveIotScreenState({
      ready: true,
      session: null
    }),
    "login"
  );
});

test("iot session view shows dashboard when auth is ready and session exists", () => {
  assert.equal(
    resolveIotScreenState({
      ready: true,
      session: {
        token: "valid-token"
      }
    }),
    "dashboard"
  );
});
