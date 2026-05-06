import net from "node:net";
import path from "node:path";

export const WINDOWS_MSVC_TARGET = "x86_64-pc-windows-msvc";
const RESTRICTED_FETCH_PORTS = new Set([
  1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95, 101,
  102, 103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143, 161, 179, 389, 427,
  465, 512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563, 587, 601, 636, 989, 990,
  993, 995, 1719, 1720, 1723, 2049, 3659, 4045, 4190, 5060, 5061, 6000, 6566, 6665, 6666, 6667,
  6668, 6669, 6697, 10080,
]);

export function resolveRustBinaryPath(rootDir, crateName, { platform = process.platform } = {}) {
  if (platform === "win32") {
    return path.join(rootDir, "target", WINDOWS_MSVC_TARGET, "debug", `${crateName}.exe`);
  }

  return path.join(rootDir, "target", "debug", crateName);
}

export function buildRustCommandEnv(
  baseEnv = process.env,
  {
    platform = process.platform,
    userProfile = baseEnv.USERPROFILE,
  } = {},
) {
  if (platform !== "win32") {
    return { ...baseEnv };
  }

  const toolchainBin = path.join(
    userProfile ?? "",
    ".rustup",
    "toolchains",
    `stable-${WINDOWS_MSVC_TARGET}`,
    "bin",
  );
  const cargoBin = path.join(userProfile ?? "", ".cargo", "bin");
  const pathParts = [toolchainBin, cargoBin, baseEnv.PATH].filter(Boolean);

  return {
    ...baseEnv,
    RUSTC: path.join(toolchainBin, "rustc.exe"),
    PATH: pathParts.join(";"),
  };
}

export function renderExampleGatewayConfig(
  source,
  { launcherUrl, ros2BackendUrl },
) {
  return source
    .replace(/launcherUrl:\s*http:\/\/127\.0\.0\.1:8790/g, `launcherUrl: ${launcherUrl}`)
    .replace(/backendBaseUrl:\s*http:\/\/ros2-backend:8080/g, `backendBaseUrl: ${ros2BackendUrl}`);
}

export async function findAvailablePort(
  startPort,
  host,
  {
    reservedPorts = new Set(),
    isPortBusy = defaultIsPortBusy,
  } = {},
) {
  let port = startPort;
  while (reservedPorts.has(port) || isRestrictedFetchPort(port) || (await isPortBusy(port, host))) {
    port += 1;
  }

  return port;
}

function isRestrictedFetchPort(port) {
  return RESTRICTED_FETCH_PORTS.has(port);
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
