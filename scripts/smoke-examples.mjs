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

  console.log(`Docker smoke scope: ${projectName}`);
  console.log(`- gateway: ${gatewayUrl}`);
  console.log(`- ros2-backend: ${ros2BackendUrl}`);
  console.log(`- flash-sale: ${flashSaleUrl}`);
  console.log(`- iot-realtime: ${iotUrl}`);
  console.log(`- ros2-bridge: ${ros2Url}`);
  console.log(`- user-module-demo: ${userModuleDemoUrl}`);

  await waitForHttpOk(`${gatewayUrl}/health`, { label: "gateway health" });
  await waitForHttpOk(`${ros2BackendUrl}/health`, { label: "ros2-backend health" });
  await waitForHttpOk(flashSaleUrl, { label: "flash-sale app" });
  await waitForHttpOk(iotUrl, { label: "iot-realtime app" });
  await waitForHttpOk(ros2Url, { label: "ros2-bridge app" });
  await waitForHttpOk(userModuleDemoUrl, { label: "user-module-demo app" });

  await runCommand("node", ["scripts/smoke-examples-local.mjs"], {
    cwd: rootDir,
    env: {
      ...process.env,
      BLADB_GATEWAY_URL: gatewayUrl,
      BLADB_FLASH_SALE_URL: flashSaleUrl,
      BLADB_IOT_URL: iotUrl,
      BLADB_ROS2_URL: ros2Url,
      BLADB_USER_MODULE_DEMO_URL: userModuleDemoUrl,
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
