import { access, mkdir, readdir } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { chromium } from "playwright-core";
import { resolveExampleStackUrls } from "./lib/example-stack.mjs";

const rootDir = process.cwd();
const artifactDir = path.join(rootDir, ".tmp", "browser-checks");
const {
  portalUrl,
  blogUrl,
  iotUrl,
  ros2Url,
  userModuleDemoUrl,
} = resolveExampleStackUrls();

const browserExecutable = await resolveBrowserExecutable();
await mkdir(artifactDir, { recursive: true });

const browser = await chromium.launch({
  headless: true,
  executablePath: browserExecutable,
});

try {
  await verifyPortal();
  await verifyBlog();
  await verifyIot();
  await verifyRos2();
  await verifyUserModuleDemo();

  const screenshots = (await readdir(artifactDir)).sort();
  console.log(JSON.stringify({ ok: true, screenshots }, null, 2));
} finally {
  await browser.close();
}

async function verifyPortal() {
  await withPage(async (page) => {
    await page.goto(portalUrl, { waitUntil: "domcontentloaded" });
    await page.waitForSelector("text=Examples Portal");
    await page.waitForSelector("text=Recommended tour");
    await page.waitForSelector("text=Blog editor");
    await screenshot(page, "portal.png");
  });
}

async function verifyBlog() {
  await withPage(async (page) => {
    await page.goto(blogUrl, { waitUntil: "domcontentloaded" });
    await page.waitForSelector("text=Published posts");
    await page.waitForSelector("text=Welcome to the Bladb blog example");
    await screenshot(page, "blog-public.png");

    await page.getByRole("button", { name: "Login", exact: true }).nth(1).click();
    await page.waitForSelector("text=Editor features unlocked");

    const title = `Browser verify post ${Date.now()}`;
    await page.locator("input").nth(2).fill(title);
    await page.locator("textarea").nth(0).fill("Browser verification summary");
    await page.locator("textarea").nth(1).fill("Browser verification body");
    await page.getByRole("button", { name: "Publish post", exact: true }).click();

    await page.waitForFunction(
      (expectedTitle) => document.body.innerText.includes(expectedTitle),
      title,
    );
    await page.waitForFunction(
      () =>
        document.body.innerText.includes("Published posts") &&
        document.body.innerText.includes("My posts"),
    );
    await screenshot(page, "blog-editor.png");
  });
}

async function verifyIot() {
  await withPage(async (page) => {
    await page.goto(iotUrl, { waitUntil: "domcontentloaded" });
    await page.waitForSelector("text=MQTT stream:");
    await page.getByRole("button", { name: "Reboot device", exact: true }).click();
    await page.waitForFunction(() => document.body.innerText.includes("MQTT stream: subscribed"));
    await page.waitForFunction(() => !document.body.innerText.includes("Last MQTT action\n--"));
    await page.waitForFunction(() => !document.body.innerText.includes("Delivered at\n--"));
    await screenshot(page, "iot-realtime.png");
  });
}

async function verifyRos2() {
  await withPage(async (page) => {
    await page.goto(ros2Url, { waitUntil: "domcontentloaded" });
    await page.waitForSelector("text=Publish Page");
    await page.getByRole("button", { name: "ros2 publish", exact: true }).click();
    await page.getByRole("button", { name: "Subscribe Page", exact: true }).click();
    await page.waitForFunction(() => !document.body.innerText.includes("Latest robot\n--"));
    await page.waitForFunction(
      () =>
        document.body.innerText.includes("RECENT MESSAGES") &&
        document.body.innerText.includes("robot-001"),
    );
    await page.waitForFunction(() => document.body.innerText.includes("anon_ros2_bridge_"));
    await screenshot(page, "ros2-bridge.png");
  });
}

async function verifyUserModuleDemo() {
  await withPage(async (page) => {
    await page.goto(userModuleDemoUrl, { waitUntil: "domcontentloaded" });
    const primary = page.locator("button.primary-button");
    await primary.waitFor({ state: "visible" });
    await page.waitForFunction(() => {
      const button = document.querySelector("button.primary-button");
      return !!button && !button.disabled && button.textContent?.trim() === "Login";
    });

    await primary.click();
    await page.waitForSelector("text=Signed in");
    await page.getByRole("button", { name: "Refresh me", exact: true }).click();
    await page.waitForSelector("text=Signed in");
    await page.getByRole("button", { name: "Logout", exact: true }).click();
    await page.waitForSelector("text=Signed out");
    await screenshot(page, "user-module-demo.png");
  });
}

async function withPage(task) {
  const context = await browser.newContext();
  const page = await context.newPage();
  try {
    await task(page);
  } finally {
    await context.close();
  }
}

async function screenshot(page, name) {
  await page.screenshot({
    path: path.join(artifactDir, name),
    fullPage: true,
  });
}

async function resolveBrowserExecutable() {
  const explicit = process.env.BLADB_BROWSER_EXECUTABLE?.trim();
  const candidates = [
    explicit,
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/usr/bin/google-chrome",
    "/usr/bin/google-chrome-stable",
    "/usr/bin/microsoft-edge",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ].filter(Boolean);

  for (const candidate of candidates) {
    try {
      await access(candidate);
      return candidate;
    } catch {
      // Try the next installed browser candidate.
    }
  }

  throw new Error(
    "No supported Chrome/Edge/Chromium executable was found. Set BLADB_BROWSER_EXECUTABLE to a local browser path.",
  );
}
