import { spawn } from "node:child_process";
import { access } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import process from "node:process";

const rootDir = process.cwd();
const gatewayHost = "127.0.0.1";
const services = [
  { name: "gateway", port: 8787 },
  { name: "flash-sale", port: 4173 },
  { name: "iot-realtime", port: 4174 },
];

const children = [];
let shuttingDown = false;

const isWindows = process.platform === "win32";
const cargoBin = isWindows ? "cargo.exe" : "cargo";
const pnpmBin = isWindows ? "pnpm.cmd" : "pnpm";
const gatewayExe = path.join(
  rootDir,
  "target",
  "debug",
  isWindows ? "bladb-gateway.exe" : "bladb-gateway",
);

try {
  for (const service of services) {
    if (await isPortBusy(service.port, gatewayHost)) {
      throw new Error(
        `port ${service.port} is already in use, stop the existing ${service.name} dev server first`,
      );
    }
  }

  await runBootstrap(cargoBin, ["build", "-p", "bladb-gateway"], "build:gateway");
  await access(gatewayExe);

  startProcess(gatewayExe, ["serve", `${gatewayHost}:8787`], "gateway");
  startProcess(
    pnpmBin,
    ["--dir", "apps/examples/flash-sale", "dev", "--host", gatewayHost, "--port", "4173"],
    "flash-sale",
  );
  startProcess(
    pnpmBin,
    ["--dir", "apps/examples/iot-realtime", "dev", "--host", gatewayHost, "--port", "4174"],
    "iot-realtime",
  );

  console.log("Bladb example stack is starting:");
  console.log("- gateway: http://127.0.0.1:8787/health");
  console.log("- flash-sale: http://127.0.0.1:4173");
  console.log("- iot-realtime: http://127.0.0.1:4174");
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

function startProcess(command, args, label) {
  const child = spawn(command, args, {
    cwd: rootDir,
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
  for (const child of children) {
    child.kill("SIGTERM");
  }

  await new Promise((resolve) => setTimeout(resolve, 200));
  process.exit(code);
}

async function isPortBusy(port, host) {
  return await new Promise((resolve) => {
    const socket = net.createConnection({ port, host });

    socket.once("connect", () => {
      socket.destroy();
      resolve(true);
    });
    socket.once("error", () => resolve(false));
  });
}

function shouldUseShell(command) {
  return isWindows && command.toLowerCase().endsWith(".cmd");
}
