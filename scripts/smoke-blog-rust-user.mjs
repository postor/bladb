import { spawn } from "node:child_process";
import process from "node:process";
import path from "node:path";
import { waitForHttpOk } from "./lib/example-stack.mjs";

const rootDir = process.cwd();
const stackCommand = process.execPath;
const stackArgs = [path.join(rootDir, "scripts", "dev-blog-rust-user.mjs")];

const stack = spawn(stackCommand, stackArgs, {
  cwd: rootDir,
  env: process.env,
  stdio: ["ignore", "pipe", "pipe"],
});

let shuttingDown = false;
let settled = false;
let stdout = "";
let stderr = "";
let gatewayUrl = "http://127.0.0.1:8788";
let rustUserServiceUrl = "http://127.0.0.1:8791";
let appUrl = "http://127.0.0.1:4180";

stack.stdout.on("data", (chunk) => {
  const text = chunk.toString();
  stdout += text;
  const matchedRustUrl = text.match(/rust-user-service:\s+(http:\/\/127\.0\.0\.1:\d+)/i)?.[1];
  if (matchedRustUrl) {
    rustUserServiceUrl = matchedRustUrl;
  }
  const matchedGatewayUrl = text.match(/gateway:\s+(http:\/\/127\.0\.0\.1:\d+)\/health/i)?.[1];
  if (matchedGatewayUrl) {
    gatewayUrl = matchedGatewayUrl;
  }
  const matchedUrl = text.match(/blog-rust-user:\s+(http:\/\/127\.0\.0\.1:\d+)/i)?.[1];
  if (matchedUrl) {
    appUrl = matchedUrl;
  }
  process.stdout.write(text);
});

stack.stderr.on("data", (chunk) => {
  const text = chunk.toString();
  stderr += text;
  process.stderr.write(text);
});

stack.on("exit", (code) => {
  if (!settled && code !== 0) {
    fail(new Error(`blog-rust-user stack exited early with code ${code ?? "unknown"}`));
  }
});

stack.on("error", (error) => {
  if (!settled) {
    fail(error);
  }
});

process.on("SIGINT", () => void shutdown(130));
process.on("SIGTERM", () => void shutdown(143));

try {
  await waitForRustUserService();
  await waitForHttpOk(`${gatewayUrl}/health`, { label: "gateway health", timeoutMs: 120_000 });
  await waitForHttpOk(appUrl, { label: "blog-rust-user app", timeoutMs: 120_000 });

  await assertHealth();
  const cookie = await assertPublicPosts();
  await assertAnonymousSession(cookie);
  const session = await assertLogin();
  await assertPublish(session.token);
  await assertOwnManageAndForeignUpdateBlocked(session.token);

  settled = true;
  console.log("blog-rust-user smoke test passed.");
  await shutdown(0);
} catch (error) {
  fail(error);
}

async function assertHealth() {
  const response = await fetch(`${gatewayUrl}/health`);
  const payload = await response.json();
  if (!response.ok || payload.ok !== true) {
    throw new Error(`gateway health returned unexpected payload: ${JSON.stringify(payload)}`);
  }

  console.log("gateway health: ok");
}

async function assertPublicPosts() {
  const response = await fetch(`${gatewayUrl}/apps/blog/posts`);
  const payload = await response.json();
  if (!response.ok || !Array.isArray(payload.data) || payload.data.length < 1) {
    throw new Error(`public posts returned unexpected payload: ${JSON.stringify(payload)}`);
  }

  const cookie = response.headers.get("set-cookie")?.split(";").at(0);
  if (!cookie) {
    throw new Error("public posts did not mint an anonymous session cookie");
  }

  console.log("public posts: ok");
  return cookie;
}

async function assertAnonymousSession(cookie) {
  const response = await fetch(`${gatewayUrl}/users/me?app=blog`, {
    headers: {
      cookie,
    },
  });
  const payload = await response.json();
  if (
    !response.ok ||
    payload.data?.anonymous !== true ||
    payload.data?.user?.app !== "blog" ||
    typeof payload.data?.user?.uid !== "string"
  ) {
    throw new Error(`anonymous me returned unexpected payload: ${JSON.stringify(payload)}`);
  }

  console.log("anonymous session: ok");
}

async function assertLogin() {
  const response = await fetch(`${gatewayUrl}/users/login`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify({
      app: "blog",
      email: "editor@blog.demo",
      password: "demo123",
    }),
  });
  const payload = await response.json();
  if (!response.ok || payload.data?.token == null || payload.data?.user?.email !== "editor@blog.demo") {
    throw new Error(`login returned unexpected payload: ${JSON.stringify(payload)}`);
  }

  console.log("login: ok");
  return payload.data;
}

async function assertPublish(token) {
  const slug = `smoke-rust-user-${Date.now()}`;
  const title = `Smoke Rust User ${Date.now()}`;

  const createResponse = await fetch(`${gatewayUrl}/execute`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      kind: "command",
      engine: "mongo",
      action: "insertOne",
      meta: {
        policy: "blog.posts.create",
      },
      collection: "posts",
      document: {
        tenantId: { $ctx: "tenantId", token: "TENANT_ID" },
        authorUid: { $ctx: "uid", token: "UID" },
        authorName: "Blog Editor",
        title,
        slug,
        summary: "Smoke test post created by the Rust user launcher.",
        body: "This verifies publish through the Rust-backed db.user flow.",
        published: true,
      },
    }),
  });
  const createPayload = await createResponse.json();
  if (!createResponse.ok || createPayload.data?.slug !== slug) {
    throw new Error(`publish returned unexpected payload: ${JSON.stringify(createPayload)}`);
  }

  const publicResponse = await fetch(`${gatewayUrl}/apps/blog/posts`);
  const publicPayload = await publicResponse.json();
  if (
    !publicResponse.ok ||
    !Array.isArray(publicPayload.data) ||
    !publicPayload.data.some((post) => post.slug === slug && post.title === title)
  ) {
    throw new Error(`published list missing smoke post: ${JSON.stringify(publicPayload)}`);
  }

  console.log("publish: ok");
}

async function assertOwnManageAndForeignUpdateBlocked(token) {
  const registerResponse = await fetch(`${gatewayUrl}/users/register`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify({
      app: "blog",
      email: `attacker-${Date.now()}@blog.demo`,
      password: "demo123",
      displayName: "Attacker Demo",
    }),
  });
  const registerPayload = await registerResponse.json();
  if (!registerResponse.ok || !registerPayload.data?.token) {
    throw new Error(`register attacker returned unexpected payload: ${JSON.stringify(registerPayload)}`);
  }

  const ownerCreateResponse = await fetch(`${gatewayUrl}/apps/blog/me/posts`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      title: `Managed Post ${Date.now()}`,
      slug: `managed-post-${Date.now()}`,
      summary: "Owned post for manage flow",
      body: "This post proves the owner can manage their own article.",
      published: true,
    }),
  });
  const ownerCreatePayload = await ownerCreateResponse.json();
  if (!ownerCreateResponse.ok || !ownerCreatePayload.data?.id) {
    throw new Error(`owner manage create returned unexpected payload: ${JSON.stringify(ownerCreatePayload)}`);
  }

  const postId = ownerCreatePayload.data.id;
  const updatedTitle = `Managed Post Updated ${Date.now()}`;
  const ownerUpdateResponse = await fetch(`${gatewayUrl}/apps/blog/me/posts/${postId}`, {
    method: "PATCH",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      title: updatedTitle,
      slug: `managed-post-updated-${Date.now()}`,
      summary: "Updated by the rightful owner",
      body: "Owner update succeeded",
      published: true,
    }),
  });
  const ownerUpdatePayload = await ownerUpdateResponse.json();
  if (!ownerUpdateResponse.ok || ownerUpdatePayload.data?.title !== updatedTitle) {
    throw new Error(`owner manage update returned unexpected payload: ${JSON.stringify(ownerUpdatePayload)}`);
  }

  const foreignUpdateResponse = await fetch(`${gatewayUrl}/apps/blog/me/posts/${postId}`, {
    method: "PATCH",
    headers: {
      authorization: `Bearer ${registerPayload.data.token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      title: "Hacked title",
      slug: "hacked-title",
      summary: "This should fail",
      body: "This should fail",
      published: true,
    }),
  });
  const foreignUpdatePayload = await foreignUpdateResponse.json();
  if (foreignUpdateResponse.ok) {
    throw new Error(`foreign update unexpectedly succeeded: ${JSON.stringify(foreignUpdatePayload)}`);
  }

  const message = JSON.stringify(foreignUpdatePayload);
  if (!message.includes("another author's article")) {
    throw new Error(`foreign update returned unexpected failure payload: ${JSON.stringify(foreignUpdatePayload)}`);
  }

  const publicResponse = await fetch(`${gatewayUrl}/apps/blog/posts`);
  const publicPayload = await publicResponse.json();
  if (
    !publicResponse.ok ||
    !Array.isArray(publicPayload.data) ||
    !publicPayload.data.some((post) => post.id === postId && post.title === updatedTitle)
  ) {
    throw new Error(`public plaza missing owner-managed article: ${JSON.stringify(publicPayload)}`);
  }

  console.log("own manage + foreign update block: ok");
}

async function waitForRustUserService() {
  const deadline = Date.now() + 120_000;
  let lastError = "unknown error";

  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${rustUserServiceUrl}/invoke/bladb.app.blog.module.user.health`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
        },
        body: JSON.stringify({
          app: "blog",
          module: "user",
          method: "health",
        }),
      });
      const payload = await response.json();
      if (response.ok && payload.ok === true && payload.data?.service === "rust-user-service") {
        console.log("rust-user-service health: ok");
        return;
      }
      lastError = JSON.stringify(payload);
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error);
    }

    await new Promise((resolve) => setTimeout(resolve, 1_000));
  }

  throw new Error(`rust-user-service health did not become ready: ${lastError}`);
}

function fail(error) {
  settled = true;
  const details = [error instanceof Error ? error.message : String(error)];
  if (stderr.trim().length > 0) {
    details.push("stack stderr:");
    details.push(stderr.trim());
  } else if (stdout.trim().length > 0) {
    details.push("stack stdout:");
    details.push(stdout.trim());
  }
  console.error(details.join("\n"));
  void shutdown(1);
}

async function shutdown(code) {
  if (shuttingDown) {
    return;
  }

  shuttingDown = true;
  await terminateProcessTree(stack.pid);
  process.exit(code);
}

async function terminateProcessTree(pid) {
  if (!pid) {
    return;
  }

  if (process.platform === "win32") {
    await new Promise((resolve) => {
      const killer = spawn("taskkill", ["/pid", String(pid), "/t", "/f"], {
        stdio: "ignore",
        shell: true,
      });
      killer.on("exit", () => resolve());
      killer.on("error", () => resolve());
    });
    return;
  }

  try {
    process.kill(pid, "SIGTERM");
  } catch {
    // Ignore already-exited children during cleanup.
  }
}
