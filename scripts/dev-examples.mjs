import process from "node:process";
import {
  dockerComposeArgs,
  runCommand,
  waitForHttpOk,
} from "./lib/example-stack.mjs";

const rootDir = process.cwd();
const composeFiles = [
  "docker/examples.compose.yaml",
  "docker/examples.dev.compose.yaml",
];
const projectName = process.env.BLADB_DOCKER_PROJECT ?? "bladb-dev";

await runCommand(
  "docker",
  dockerComposeArgs({
    projectName,
    composeFiles,
    commandArgs: ["up", "--build", "-d", "--remove-orphans"],
  }),
  { cwd: rootDir },
);

await waitForHttpOk("http://127.0.0.1:8787/health", { label: "gateway health" });
await waitForHttpOk("http://127.0.0.1:4173", { label: "flash-sale app" });
await waitForHttpOk("http://127.0.0.1:4174", { label: "iot-realtime app" });

console.log(`Docker dev scope: ${projectName}`);
console.log("- gateway: http://127.0.0.1:8787/health");
console.log("- flash-sale: http://127.0.0.1:4173");
console.log("- iot-realtime: http://127.0.0.1:4174");
console.log("Stop with: pnpm dev:examples:down");
