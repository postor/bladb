#!/usr/bin/env node

import process from "node:process";
import path from "node:path";
import { startHttpServerModules } from "./index.ts";

const args = process.argv.slice(2);
const options = parseArgs(args);

const started = await startHttpServerModules({
  app: options.app,
  modulesDir: path.resolve(options.modulesDir),
  host: options.host,
  port: options.port,
});

console.log(`Bladb server modules listening on ${started.baseUrl}`);
for (const subject of started.subjects) {
  console.log(`- ${subject}`);
}

process.on("SIGINT", async () => {
  await started.transport.close();
  process.exit(0);
});

process.on("SIGTERM", async () => {
  await started.transport.close();
  process.exit(0);
});

function parseArgs(argv) {
  const options = {
    app: process.env.BLADB_SERVER_APP ?? "user-module-demo",
    modulesDir: process.env.BLADB_SERVER_MODULES_DIR ?? "apps/server-modules",
    host: process.env.BLADB_SERVER_HOST ?? "127.0.0.1",
    port: Number.parseInt(process.env.BLADB_SERVER_PORT ?? "8790", 10),
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--app") {
      options.app = argv[++index] ?? options.app;
      continue;
    }
    if (arg === "--modules-dir") {
      options.modulesDir = argv[++index] ?? options.modulesDir;
      continue;
    }
    if (arg === "--host") {
      options.host = argv[++index] ?? options.host;
      continue;
    }
    if (arg === "--port") {
      options.port = Number.parseInt(argv[++index] ?? String(options.port), 10);
      continue;
    }
  }

  return options;
}
