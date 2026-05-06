import { spawn } from "node:child_process";
import { access } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import {
  EXAMPLE_STACK_HOST,
  clearExampleStackState,
  exampleStackPortEnv,
  exampleStackUrlsFromPorts,
  resolveExampleStackPorts,
  writeExampleStackState,
} from "./lib/example-stack.mjs";

const rootDir = process.cwd();
const children = [];
let shuttingDown = false;

const isWindows = process.platform === "win32";
const cargoBin = isWindows ? "C:/Users/posto/.cargo/bin/cargo.exe" : "cargo";
const pnpmBin = isWindows ? "pnpm.cmd" : "pnpm";
const nodeBin = isWindows ? "node.exe" : "node";
const gatewayExe = path.join(
  rootDir,
  "target",
  "debug",
  isWindows ? "bladb-gateway.exe" : "bladb-gateway",
);

try {
  const ports = await resolveExampleStackPorts();
  const urls = exampleStackUrlsFromPorts(ports);
  const sharedEnv = {
    ...process.env,
    ...exampleStackPortEnv(ports),
    VITE_EXAMPLE_FLASH_SALE_URL: urls.flashSaleUrl,
    VITE_EXAMPLE_BLOG_URL: urls.blogUrl,
    VITE_EXAMPLE_IOT_URL: urls.iotUrl,
    VITE_EXAMPLE_ROS2_URL: urls.ros2Url,
    VITE_EXAMPLE_USER_MODULE_DEMO_URL: urls.userModuleDemoUrl,
  };
  await clearExampleStackState();

  await runBootstrap(cargoBin, ["build", "-p", "bladb-gateway"], "build:gateway");
  await access(gatewayExe);

  startProcess(
    nodeBin,
    [
      "packages/server/src/cli.mjs",
      "--app",
      "user-module-demo",
      "--modules-dir",
      "apps/server-modules",
      "--host",
      EXAMPLE_STACK_HOST,
      "--port",
      "8790",
    ],
    "server-modules",
    sharedEnv,
  );
  startProcess(gatewayExe, ["serve", `${EXAMPLE_STACK_HOST}:${ports.gateway}`], "gateway");
  startProcess(
    pnpmBin,
    [
      "--dir",
      "apps/examples/examples-portal",
      "dev",
      "--host",
      EXAMPLE_STACK_HOST,
      "--port",
      String(ports.portal),
    ],
    "examples-portal",
    {
      ...sharedEnv,
      VITE_BLADB_URL: urls.gatewayUrl,
      VITE_EXAMPLE_PORTAL_URL: urls.portalUrl,
    },
  );
  startProcess(
    pnpmBin,
    [
      "--dir",
      "apps/examples/flash-sale",
      "dev",
      "--host",
      EXAMPLE_STACK_HOST,
      "--port",
      String(ports.flashSale),
    ],
    "flash-sale",
    {
      ...sharedEnv,
      VITE_BLADB_URL: urls.gatewayUrl,
      VITE_EXAMPLE_PORTAL_URL: urls.portalUrl,
    },
  );
  startProcess(
    pnpmBin,
    [
      "--dir",
      "apps/examples/blog",
      "dev",
      "--host",
      EXAMPLE_STACK_HOST,
      "--port",
      String(ports.blog),
    ],
    "blog",
    {
      ...sharedEnv,
      VITE_BLADB_URL: urls.gatewayUrl,
      VITE_EXAMPLE_PORTAL_URL: urls.portalUrl,
    },
  );
  startProcess(
    pnpmBin,
    [
      "--dir",
      "apps/examples/iot-realtime",
      "dev",
      "--host",
      EXAMPLE_STACK_HOST,
      "--port",
      String(ports.iot),
    ],
    "iot-realtime",
    {
      ...sharedEnv,
      VITE_BLADB_URL: urls.gatewayUrl,
      VITE_EXAMPLE_PORTAL_URL: urls.portalUrl,
    },
  );
  startProcess(
    pnpmBin,
    [
      "--dir",
      "apps/examples/ros2-bridge",
      "dev",
      "--host",
      EXAMPLE_STACK_HOST,
      "--port",
      String(ports.ros2),
    ],
    "ros2-bridge",
    {
      ...sharedEnv,
      VITE_BLADB_URL: urls.gatewayUrl,
      VITE_EXAMPLE_PORTAL_URL: urls.portalUrl,
    },
  );
  startProcess(
    pnpmBin,
    [
      "--dir",
      "apps/examples/user-module-demo",
      "dev",
      "--host",
      EXAMPLE_STACK_HOST,
      "--port",
      String(ports.userModuleDemo),
    ],
    "user-module-demo",
    {
      ...sharedEnv,
      VITE_BLADB_URL: urls.gatewayUrl,
      VITE_EXAMPLE_PORTAL_URL: urls.portalUrl,
    },
  );
  await writeExampleStackState({ ports, source: "local-dev" });

  console.log("Bladb example stack is starting:");
  console.log(`- server-modules: http://${EXAMPLE_STACK_HOST}:8790`);
  console.log(`- gateway: ${urls.gatewayUrl}/health`);
  console.log(`- examples-portal: ${urls.portalUrl}`);
  console.log(`- flash-sale: ${urls.flashSaleUrl}`);
  console.log(`- blog: ${urls.blogUrl}`);
  console.log(`- iot-realtime: ${urls.iotUrl}`);
  console.log(`- ros2-bridge: ${urls.ros2Url}`);
  console.log(`- user-module-demo: ${urls.userModuleDemoUrl}`);
} catch (error) {
  console.error(error.message);
  await shutdown(1);
}

process.on("SIGINT", () => void shutdown(0));
process.on("SIGTERM", () => void shutdown(0));
process.on("exit", () => {
  if (!shuttingDown) {
    for (const child of children) {
      child.kill("SIGTERM");
    }
  }
});

async function runBootstrap(command, args, label) {
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: rootDir,
      stdio: "inherit",
      shell: shouldUseShell(command),
    });

    child.on("exit", (code) => {
      if (code === 0) {
        resolve();
        return;
      }

      reject(new Error(`${label} exited with code ${code ?? "unknown"}`));
    });
    child.on("error", reject);
  });
}

function startProcess(command, args, label, env = process.env) {
  const child = spawn(command, args, {
    cwd: rootDir,
    env,
    stdio: "inherit",
    shell: shouldUseShell(command),
  });

  child.on("exit", (code) => {
    if (!shuttingDown && code !== 0) {
      console.error(`${label} exited with code ${code ?? "unknown"}`);
      void shutdown(code ?? 1);
    }
  });
  child.on("error", (error) => {
    console.error(`${label} failed: ${error.message}`);
    void shutdown(1);
  });

  children.push(child);
}

async function shutdown(code) {
  if (shuttingDown) {
    return;
  }

  shuttingDown = true;
  await clearExampleStackState();
  for (const child of children) {
    child.kill("SIGTERM");
  }

  await new Promise((resolve) => setTimeout(resolve, 200));
  process.exit(code);
}

function shouldUseShell(command) {
  return isWindows && command.toLowerCase().endsWith(".cmd");
}
