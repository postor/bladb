import http from "node:http";

import type { ServerModuleTransport } from "./launcher.ts";

export interface CreateHttpServerModuleTransportOptions {
  host?: string;
  port: number;
}

export interface HttpServerModuleTransport extends ServerModuleTransport {
  baseUrl(): string;
  close(): Promise<void>;
}

export interface StartHttpServerModulesOptions extends CreateHttpServerModuleTransportOptions {
  app: string;
  modulesDir: string;
}

export async function createHttpServerModuleTransport(
  options: CreateHttpServerModuleTransportOptions,
): Promise<HttpServerModuleTransport> {
  const host = options.host ?? "127.0.0.1";
  const handlers = new Map<string, (payload: unknown) => Promise<unknown>>();

  const server = http.createServer(async (request, response) => {
    try {
      if (request.method !== "POST" || !request.url?.startsWith("/invoke/")) {
        response.writeHead(404, { "content-type": "application/json" });
        response.end(JSON.stringify({ ok: false, code: "NOT_FOUND", message: "route not found" }));
        return;
      }

      const subject = decodeURIComponent(request.url.slice("/invoke/".length));
      const handler = handlers.get(subject);
      if (!handler) {
        response.writeHead(404, { "content-type": "application/json" });
        response.end(
          JSON.stringify({
            ok: false,
            code: "SUBJECT_NOT_FOUND",
            message: `no handler registered for ${subject}`,
          }),
        );
        return;
      }

      const body = await readJsonBody(request);
      const result = await handler(body);
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify(result));
    } catch (error) {
      response.writeHead(500, { "content-type": "application/json" });
      response.end(
        JSON.stringify({
          ok: false,
          code: "HTTP_TRANSPORT_ERROR",
          message: error instanceof Error ? error.message : "unknown http transport error",
        }),
      );
    }
  });

  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(options.port, host, () => {
      server.off("error", reject);
      resolve();
    });
  });

  return {
    async subscribe(subject, handler) {
      handlers.set(subject, handler);
    },
    baseUrl() {
      const address = server.address();
      if (!address || typeof address === "string") {
        throw new Error("http server transport is not bound to a tcp address");
      }

      return `http://${host}:${address.port}`;
    },
    async close() {
      await new Promise<void>((resolve, reject) => {
        server.close((error) => {
          if (error) {
            reject(error);
            return;
          }

          resolve();
        });
      });
    },
  };
}

export async function startHttpServerModules(
  options: StartHttpServerModulesOptions,
): Promise<{
  transport: HttpServerModuleTransport;
  subjects: string[];
  baseUrl: string;
}> {
  const { startServerModules } = await import("./launcher.ts");
  const transport = await createHttpServerModuleTransport(options);
  const started = await startServerModules({
    app: options.app,
    modulesDir: options.modulesDir,
    transport,
  });

  return {
    transport,
    subjects: started.subjects,
    baseUrl: transport.baseUrl(),
  };
}

async function readJsonBody(request: http.IncomingMessage): Promise<unknown> {
  const chunks: Buffer[] = [];
  for await (const chunk of request) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }

  if (chunks.length === 0) {
    return {};
  }

  return JSON.parse(Buffer.concat(chunks).toString("utf8"));
}
