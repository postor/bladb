import process from "node:process";
import {
  dockerComposeArgs,
  exampleStackPortEnv,
  resolveExampleStackPorts,
  runCommand,
} from "./lib/example-stack.mjs";

const rootDir = process.cwd();
const composeFiles = [
  "docker/examples.compose.yaml",
  "docker/examples.dev.compose.yaml",
];
const projectName = process.env.BLADB_DOCKER_PROJECT ?? "bladb-dev";
const ports = await resolveExampleStackPorts().catch(() => null);
const composeEnv = {
  ...process.env,
  ...(ports ? exampleStackPortEnv(ports) : {}),
};

await runCommand(
  "docker",
  dockerComposeArgs({
    projectName,
    composeFiles,
    commandArgs: ["down", "--volumes", "--remove-orphans"],
  }),
  { cwd: rootDir, env: composeEnv },
);
