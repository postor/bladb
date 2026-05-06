import crypto from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { readFileSync } from "node:fs";
import net from "node:net";
import { spawn } from "node:child_process";
import path from "node:path";

export const EXAMPLE_STACK_HOST = "127.0.0.1";
export const EXAMPLE_STACK_STATE_PATH = path.join(".tmp", "example-stack-state.json");

const EXAMPLE_STACK_SERVICES = [
  {
    key: "gateway",
    urlKey: "gatewayUrl",
    label: "gateway",
    defaultPort: 8787,
    portEnv: "BLADB_GATEWAY_PORT",
    urlEnv: "BLADB_GATEWAY_URL",
  },
  {
    key: "ros2Backend",
    urlKey: "ros2BackendUrl",
    label: "ros2-backend",
    defaultPort: 8080,
    portEnv: "BLADB_ROS2_BACKEND_PORT",
    urlEnv: "BLADB_ROS2_BACKEND_URL",
  },
  {
    key: "portal",
    urlKey: "portalUrl",
    label: "examples-portal",
    defaultPort: 4172,
    portEnv: "BLADB_EXAMPLES_PORTAL_PORT",
    urlEnv: "BLADB_EXAMPLES_PORTAL_URL",
  },
  {
    key: "flashSale",
    urlKey: "flashSaleUrl",
    label: "flash-sale",
    defaultPort: 4173,
    portEnv: "BLADB_FLASH_SALE_PORT",
    urlEnv: "BLADB_FLASH_SALE_URL",
  },
  {
    key: "blog",
    urlKey: "blogUrl",
    label: "blog",
    defaultPort: 4174,
    portEnv: "BLADB_BLOG_PORT",
    urlEnv: "BLADB_BLOG_URL",
  },
  {
    key: "iot",
    urlKey: "iotUrl",
    label: "iot-realtime",
    defaultPort: 4175,
    portEnv: "BLADB_IOT_PORT",
    urlEnv: "BLADB_IOT_URL",
  },
  {
    key: "ros2",
    urlKey: "ros2Url",
    label: "ros2-bridge",
    defaultPort: 4176,
    portEnv: "BLADB_ROS2_PORT",
    urlEnv: "BLADB_ROS2_URL",
  },
  {
    key: "userModuleDemo",
    urlKey: "userModuleDemoUrl",
    label: "user-module-demo",
    defaultPort: 4177,
    portEnv: "BLADB_USER_MODULE_DEMO_PORT",
    urlEnv: "BLADB_USER_MODULE_DEMO_URL",
  },
];

export function resolveExampleStackUrls(env = process.env, host = EXAMPLE_STACK_HOST) {
  const persisted = readPersistedExampleStackStateSync();
  const urls = {};

  for (const service of EXAMPLE_STACK_SERVICES) {
    const explicitUrl = normalizedString(env[service.urlEnv]);
    if (explicitUrl) {
      urls[service.urlKey] = explicitUrl;
      continue;
    }

    const explicitPort = parsePortEnv(env[service.portEnv], service.portEnv);
    const persistedPort = persisted?.ports?.[service.key];
    urls[service.urlKey] = formatLocalUrl(host, explicitPort ?? persistedPort ?? service.defaultPort);
  }

  return urls;
}

export async function resolveExampleStackPorts({
  env = process.env,
  host = EXAMPLE_STACK_HOST,
  isPortBusy = defaultIsPortBusy,
} = {}) {
  const assignedPorts = new Set();
  const ports = {};

  for (const service of EXAMPLE_STACK_SERVICES) {
    const explicitPort = parsePortEnv(env[service.portEnv], service.portEnv);
    if (explicitPort != null) {
      if (assignedPorts.has(explicitPort)) {
        throw new Error(
          `${service.portEnv} conflicts with another example stack service on ${host}:${explicitPort}`,
        );
      }

      if (await isPortBusy(explicitPort, host)) {
        throw new Error(`${service.portEnv} is already in use on ${host}:${explicitPort}`);
      }

      assignedPorts.add(explicitPort);
      ports[service.key] = explicitPort;
      continue;
    }

    let candidatePort = service.defaultPort;
    while (assignedPorts.has(candidatePort) || (await isPortBusy(candidatePort, host))) {
      candidatePort += 1;
    }

    assignedPorts.add(candidatePort);
    ports[service.key] = candidatePort;
  }

  return ports;
}

export function exampleStackUrlsFromPorts(ports, host = EXAMPLE_STACK_HOST) {
  return {
    gatewayUrl: formatLocalUrl(host, ports.gateway),
    ros2BackendUrl: formatLocalUrl(host, ports.ros2Backend),
    portalUrl: formatLocalUrl(host, ports.portal),
    flashSaleUrl: formatLocalUrl(host, ports.flashSale),
    blogUrl: formatLocalUrl(host, ports.blog),
    iotUrl: formatLocalUrl(host, ports.iot),
    ros2Url: formatLocalUrl(host, ports.ros2),
    userModuleDemoUrl: formatLocalUrl(host, ports.userModuleDemo),
  };
}

export function exampleStackPortEnv(ports) {
  return {
    BLADB_GATEWAY_PORT: String(ports.gateway),
    BLADB_ROS2_BACKEND_PORT: String(ports.ros2Backend),
    BLADB_EXAMPLES_PORTAL_PORT: String(ports.portal),
    BLADB_FLASH_SALE_PORT: String(ports.flashSale),
    BLADB_BLOG_PORT: String(ports.blog),
    BLADB_IOT_PORT: String(ports.iot),
    BLADB_ROS2_PORT: String(ports.ros2),
    BLADB_USER_MODULE_DEMO_PORT: String(ports.userModuleDemo),
  };
}

export function exampleStackUrlEnv(urls) {
  return {
    BLADB_GATEWAY_URL: urls.gatewayUrl,
    BLADB_ROS2_BACKEND_URL: urls.ros2BackendUrl,
    BLADB_EXAMPLES_PORTAL_URL: urls.portalUrl,
    BLADB_FLASH_SALE_URL: urls.flashSaleUrl,
    BLADB_BLOG_URL: urls.blogUrl,
    BLADB_IOT_URL: urls.iotUrl,
    BLADB_ROS2_URL: urls.ros2Url,
    BLADB_USER_MODULE_DEMO_URL: urls.userModuleDemoUrl,
  };
}

export async function writeExampleStackState({ ports, projectName = null, source }) {
  await mkdir(path.dirname(EXAMPLE_STACK_STATE_PATH), { recursive: true });
  await writeFile(
    EXAMPLE_STACK_STATE_PATH,
    JSON.stringify(
      {
        source,
        projectName,
        ports,
        updatedAt: new Date().toISOString(),
      },
      null,
      2,
    ),
    "utf8",
  );
}

export async function clearExampleStackState() {
  await rm(EXAMPLE_STACK_STATE_PATH, { force: true });
}

export async function readExampleStackState() {
  try {
    const raw = await readFile(EXAMPLE_STACK_STATE_PATH, "utf8");
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export function parseDockerComposePort(output) {
  const trimmed = output.trim();
  const match = trimmed.match(/:(\d+)\s*$/);
  if (!match) {
    throw new Error(`unable to parse docker compose port output: ${trimmed}`);
  }

  return `http://127.0.0.1:${match[1]}`;
}

export function createScopedProjectName(prefix = "bladb-smoke") {
  return `${prefix}-${Date.now().toString(36)}-${crypto.randomBytes(2).toString("hex")}`;
}

export function dockerComposeArgs({ projectName, composeFiles, commandArgs }) {
  return [
    "compose",
    ...composeFiles.flatMap((composeFile) => ["-f", composeFile]),
    "-p",
    projectName,
    ...commandArgs,
  ];
}

export async function runCommand(command, args, options = {}) {
  const {
    cwd,
    env,
    stdio = "inherit",
    captureOutput = false,
  } = options;

  return await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      env,
      stdio: captureOutput ? ["ignore", "pipe", "pipe"] : stdio,
    });

    let stdout = "";
    let stderr = "";

    if (captureOutput) {
      child.stdout?.on("data", (chunk) => {
        stdout += chunk.toString();
      });
      child.stderr?.on("data", (chunk) => {
        stderr += chunk.toString();
      });
    }

    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) {
        resolve({ stdout, stderr });
        return;
      }

      reject(
        new Error(
          stderr.trim() || `${command} ${args.join(" ")} exited with code ${code ?? "unknown"}`,
        ),
      );
    });
  });
}

export async function resolveComposeServiceUrl({
  workdir,
  projectName,
  composeFiles,
  service,
  containerPort,
}) {
  const { stdout } = await runCommand(
    "docker",
    dockerComposeArgs({
      projectName,
      composeFiles,
      commandArgs: ["port", service, String(containerPort)],
    }),
    {
      cwd: workdir,
      captureOutput: true,
    },
  );

  return parseDockerComposePort(stdout);
}

export async function waitForHttpOk(
  url,
  {
    label = url,
    timeoutMs = 60_000,
    intervalMs = 1_000,
  } = {},
) {
  const deadline = Date.now() + timeoutMs;
  let lastError;

  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) {
        return;
      }

      lastError = new Error(`${label} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }

    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }

  throw new Error(`${label} did not become ready: ${lastError?.message ?? "unknown error"}`);
}

async function defaultIsPortBusy(port, host) {
  return await new Promise((resolve) => {
    const socket = net.createConnection({ port, host });

    socket.once("connect", () => {
      socket.destroy();
      resolve(true);
    });
    socket.once("error", () => resolve(false));
  });
}

function normalizedString(value) {
  if (typeof value !== "string") {
    return null;
  }

  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function parsePortEnv(value, envName) {
  const normalized = normalizedString(value);
  if (normalized == null) {
    return null;
  }

  const port = Number.parseInt(normalized, 10);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error(`${envName} must be an integer between 1 and 65535`);
  }

  return port;
}

function formatLocalUrl(host, port) {
  return `http://${host}:${port}`;
}

function readPersistedExampleStackStateSync() {
  try {
    return JSON.parse(readFileSync(EXAMPLE_STACK_STATE_PATH, "utf8"));
  } catch {
    return null;
  }
}
