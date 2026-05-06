import { spawn } from "node:child_process";
import process from "node:process";
import { execFileSync } from "node:child_process";
import { readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import {
  buildRustCommandEnv,
  findAvailablePort,
  resolveRustBinaryPath,
} from "./lib/local-rust-dev.mjs";

const rootDir = process.cwd();
const children = [];
let shuttingDown = false;

const isWindows = process.platform === "win32";
const cargoBin = isWindows ? "C:\\Users\\posto\\.cargo\\bin\\cargo.exe" : "cargo";
const pnpmBin = isWindows ? "pnpm.cmd" : "pnpm";
const baseEnv = buildRustCommandEnv();
const host = "127.0.0.1";
const rustUserServicePort = await findAvailablePort(8791, host);
const gatewayPort = await findAvailablePort(8788, host, { reservedPorts: new Set([rustUserServicePort]) });
const frontendPort = await findAvailablePort(
  4180,
  host,
  { reservedPorts: new Set([rustUserServicePort, gatewayPort]) },
);
const generatedConfigPath = path.join(rootDir, "bladb.rust-user-blog.generated.yml");

const env = {
  ...baseEnv,
  BLADB_SERVER_HOST: host,
  BLADB_SERVER_PORT: String(rustUserServicePort),
  VITE_BLADB_URL: `http://${host}:${gatewayPort}`,
  BLADB_BLOG_RUST_USER_URL: `http://${host}:${frontendPort}`,
};

runInstall();
runRustBuilds();
await writeGeneratedGatewayConfig();

const rustUserServiceExe = resolveRustBinaryPath(rootDir, "rust-user-service");
const gatewayExe = resolveRustBinaryPath(rootDir, "bladb-gateway");

startProcess(rustUserServiceExe, [], "rust-user-service", env);
startProcess(
  gatewayExe,
  ["serve", `${host}:${gatewayPort}`, generatedConfigPath],
  "gateway-rust-user-blog",
  env
);
startProcess(
  pnpmBin,
  ["--dir", "apps/examples/blog-rust-user", "dev", "--host", host, "--port", String(frontendPort), "--strictPort"],
  "blog-rust-user",
  env
);

console.log("Blog Rust user stack is starting:");
console.log(`- rust-user-service: http://${host}:${rustUserServicePort}`);
console.log(`- gateway: http://${host}:${gatewayPort}/health`);
console.log(`- blog-rust-user: http://${host}:${frontendPort}`);

process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));

function startProcess(command, args, label, childEnv) {
  const child = spawn(command, args, {
    cwd: rootDir,
    env: childEnv,
    stdio: "inherit",
    shell: isWindows && command.toLowerCase().endsWith(".cmd")
  });

  child.on("exit", (code) => {
    if (!shuttingDown && code !== 0) {
      console.error(`${label} exited with code ${code ?? "unknown"}`);
      shutdown(code ?? 1);
    }
  });
  child.on("error", (error) => {
    console.error(`${label} failed: ${error.message}`);
    shutdown(1);
  });

  children.push(child);
}

function shutdown(code) {
  if (shuttingDown) {
    return;
  }

  shuttingDown = true;
  for (const child of children) {
    child.kill("SIGTERM");
  }
  setTimeout(async () => {
    await rm(generatedConfigPath, { force: true });
    process.exit(code);
  }, 200);
}

function runInstall() {
  const command = isWindows ? "pnpm.cmd" : "pnpm";
  execFileSync(command, ["install"], {
    cwd: rootDir,
    env,
    stdio: "inherit",
    shell: isWindows
  });
}

function runRustBuilds() {
  execFileSync(cargoBin, ["build", "-p", "rust-user-service", "-p", "bladb-gateway"], {
    cwd: rootDir,
    env,
    stdio: "inherit"
  });
}

async function writeGeneratedGatewayConfig() {
  const source = await readFile(path.join(rootDir, "bladb.rust-user-blog.yml"), "utf8");
  const rendered = source.replace(
    /launcherUrl:\s*http:\/\/127\.0\.0\.1:\d+/g,
    `launcherUrl: http://${host}:${rustUserServicePort}`,
  );
  await writeFile(generatedConfigPath, rendered, "utf8");
}
