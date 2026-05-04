import process from "node:process";
import { dockerComposeArgs, runCommand } from "./lib/example-stack.mjs";

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
    commandArgs: ["down", "--volumes", "--remove-orphans"],
  }),
  { cwd: rootDir },
);
