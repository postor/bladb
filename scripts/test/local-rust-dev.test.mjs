import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import {
  buildRustCommandEnv,
  findAvailablePort,
  renderExampleGatewayConfig,
  resolveRustBinaryPath,
  WINDOWS_MSVC_TARGET,
} from "../lib/local-rust-dev.mjs";

test("resolveRustBinaryPath uses the MSVC target directory on Windows", () => {
  assert.equal(
    resolveRustBinaryPath("D:/study/bladb", "bladb-gateway", { platform: "win32" }),
    path.join(
      "D:/study/bladb",
      "target",
      WINDOWS_MSVC_TARGET,
      "debug",
      "bladb-gateway.exe",
    ),
  );
});

test("resolveRustBinaryPath keeps the default cargo target layout outside Windows", () => {
  assert.equal(
    resolveRustBinaryPath("/workspace/bladb", "rust-user-service", { platform: "linux" }),
    path.join("/workspace/bladb", "target", "debug", "rust-user-service"),
  );
});

test("buildRustCommandEnv pins the Windows toolchain binaries ahead of PATH", () => {
  const env = buildRustCommandEnv(
    { PATH: "C:\\existing\\bin" },
    { platform: "win32", userProfile: "C:\\Users\\posto" },
  );

  assert.equal(
    env.RUSTC,
    path.join(
      "C:\\Users\\posto",
      ".rustup",
      "toolchains",
      "stable-x86_64-pc-windows-msvc",
      "bin",
      "rustc.exe",
    ),
  );
  assert.match(
    env.PATH,
    /^C:\\Users\\posto\\\.rustup\\toolchains\\stable-x86_64-pc-windows-msvc\\bin;C:\\Users\\posto\\\.cargo\\bin;C:\\existing\\bin$/,
  );
});

test("renderExampleGatewayConfig rewrites launcher and ros2 backend URLs", () => {
  const rendered = renderExampleGatewayConfig(
    [
      "launcherUrl: http://127.0.0.1:8790",
      "backendBaseUrl: http://ros2-backend:8080",
    ].join("\n"),
    {
      launcherUrl: "http://127.0.0.1:18790",
      ros2BackendUrl: "http://127.0.0.1:18080",
    },
  );

  assert.equal(
    rendered,
    [
      "launcherUrl: http://127.0.0.1:18790",
      "backendBaseUrl: http://127.0.0.1:18080",
    ].join("\n"),
  );
});

test("findAvailablePort skips restricted browser ports", async () => {
  const port = await findAvailablePort(4190, "127.0.0.1", {
    isPortBusy: async () => false,
  });

  assert.equal(port, 4191);
});
