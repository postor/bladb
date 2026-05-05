import crypto from "node:crypto";
import { spawn } from "node:child_process";

export function resolveExampleStackUrls(env = process.env) {
  return {
    gatewayUrl: env.BLADB_GATEWAY_URL ?? "http://127.0.0.1:8787",
    flashSaleUrl: env.BLADB_FLASH_SALE_URL ?? "http://127.0.0.1:4173",
    iotUrl: env.BLADB_IOT_URL ?? "http://127.0.0.1:4174",
    ros2Url: env.BLADB_ROS2_URL ?? "http://127.0.0.1:4175",
  };
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
