import process from "node:process";
import {
  dockerComposeArgs,
  exampleStackPortEnv,
  resolveExampleStackPorts,
  resolveComposeServiceUrl,
  runCommand,
  waitForHttpOk,
} from "./lib/example-stack.mjs";

const rootDir = process.cwd();
const composeFiles = [
  "docker/examples.compose.yaml",
  "docker/examples.dev.compose.yaml",
];
const projectName = process.env.BLADB_DOCKER_PROJECT ?? "bladb-dev";
const ports = await resolveExampleStackPorts();
const composeEnv = {
  ...process.env,
  ...exampleStackPortEnv(ports),
};

await runCommand(
  "docker",
  dockerComposeArgs({
    projectName,
    composeFiles,
    commandArgs: ["up", "--build", "-d", "--remove-orphans"],
  }),
  { cwd: rootDir, env: composeEnv },
);

const gatewayUrl = await resolveComposeServiceUrl({
  workdir: rootDir,
  projectName,
  composeFiles,
  service: "gateway",
  containerPort: 8787,
});
const ros2BackendUrl = await resolveComposeServiceUrl({
  workdir: rootDir,
  projectName,
  composeFiles,
  service: "ros2-backend",
  containerPort: 8080,
});
const flashSaleUrl = await resolveComposeServiceUrl({
  workdir: rootDir,
  projectName,
  composeFiles,
  service: "flash-sale",
  containerPort: 80,
});
const iotUrl = await resolveComposeServiceUrl({
  workdir: rootDir,
  projectName,
  composeFiles,
  service: "iot-realtime",
  containerPort: 80,
});
const ros2Url = await resolveComposeServiceUrl({
  workdir: rootDir,
  projectName,
  composeFiles,
  service: "ros2-bridge",
  containerPort: 80,
});
const userModuleDemoUrl = await resolveComposeServiceUrl({
  workdir: rootDir,
  projectName,
  composeFiles,
  service: "user-module-demo",
  containerPort: 80,
});

await waitForHttpOk(`${gatewayUrl}/health`, { label: "gateway health" });
await waitForHttpOk(`${ros2BackendUrl}/health`, { label: "ros2-backend health" });
await waitForHttpOk(flashSaleUrl, { label: "flash-sale app" });
await waitForHttpOk(iotUrl, { label: "iot-realtime app" });
await waitForHttpOk(ros2Url, { label: "ros2-bridge app" });
await waitForHttpOk(userModuleDemoUrl, { label: "user-module-demo app" });

console.log(`Docker dev scope: ${projectName}`);
console.log(`- gateway: ${gatewayUrl}/health`);
console.log(`- ros2-backend: ${ros2BackendUrl}/health`);
console.log(`- flash-sale: ${flashSaleUrl}`);
console.log(`- iot-realtime: ${iotUrl}`);
console.log(`- ros2-bridge: ${ros2Url}`);
console.log(`- user-module-demo: ${userModuleDemoUrl}`);
console.log("Stop with: pnpm dev:examples:down");
