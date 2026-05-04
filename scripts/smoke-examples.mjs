import process from "node:process";
import {
  createScopedProjectName,
  dockerComposeArgs,
  resolveComposeServiceUrl,
  runCommand,
  waitForHttpOk,
} from "./lib/example-stack.mjs";

const rootDir = process.cwd();
const composeFiles = [
  "docker/examples.compose.yaml",
  "docker/examples.smoke.compose.yaml",
];
const projectName = createScopedProjectName();

let stackStarted = false;

try {
  await runCommand(
    "docker",
    dockerComposeArgs({
      projectName,
      composeFiles,
      commandArgs: ["up", "--build", "-d", "--remove-orphans"],
    }),
    { cwd: rootDir },
  );
  stackStarted = true;

  const gatewayUrl = await resolveComposeServiceUrl({
    workdir: rootDir,
    projectName,
    composeFiles,
    service: "gateway",
    containerPort: 8787,
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

  console.log(`Docker smoke scope: ${projectName}`);
  console.log(`- gateway: ${gatewayUrl}`);
  console.log(`- flash-sale: ${flashSaleUrl}`);
  console.log(`- iot-realtime: ${iotUrl}`);

  await waitForHttpOk(`${gatewayUrl}/health`, { label: "gateway health" });
  await waitForHttpOk(flashSaleUrl, { label: "flash-sale app" });
  await waitForHttpOk(iotUrl, { label: "iot-realtime app" });

  await runCommand("node", ["scripts/smoke-examples-local.mjs"], {
    cwd: rootDir,
    env: {
      ...process.env,
      BLADB_GATEWAY_URL: gatewayUrl,
      BLADB_FLASH_SALE_URL: flashSaleUrl,
      BLADB_IOT_URL: iotUrl,
    },
  });
} finally {
  if (stackStarted) {
    try {
      await runCommand(
        "docker",
        dockerComposeArgs({
          projectName,
          composeFiles,
          commandArgs: ["down", "--volumes", "--remove-orphans"],
        }),
        { cwd: rootDir },
      );
    } catch (error) {
      console.error(`failed to clean docker smoke scope ${projectName}: ${error.message}`);
    }
  }
}
