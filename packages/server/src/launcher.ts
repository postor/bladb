import { pathToFileURL } from "node:url";
import { readdir } from "node:fs/promises";
import path from "node:path";
import { withServerModuleDbScope } from "./db.ts";

const MODULE_EXTENSIONS = new Set([".ts", ".mts", ".js", ".mjs"]);

export interface DiscoverServerModulesOptions {
  modulesDir: string;
}

export interface DiscoveredServerModule {
  moduleName: string;
  filePath: string;
  methods: string[];
}

export interface ServerModuleInvocation {
  app: string;
  module: string;
  method: string;
  input?: unknown;
  requestId?: string;
  db?: Record<string, unknown>;
  meta?: {
    traceId?: string;
  };
}

export interface ServerModuleRegistry {
  listMethodsForModule(moduleName: string): string[];
  invoke(invocation: ServerModuleInvocation): Promise<unknown>;
}

export interface ServerModuleTransport {
  subscribe(subject: string, handler: (payload: unknown) => Promise<unknown>): Promise<void>;
}

export interface CreateServerModuleLauncherOptions extends DiscoverServerModulesOptions {
  app: string;
  transport: ServerModuleTransport;
}

export interface ServerModuleLauncher {
  start(): Promise<string[]>;
}

export interface StartServerModulesOptions extends CreateServerModuleLauncherOptions {}

export interface StartedServerModules {
  launcher: ServerModuleLauncher;
  subjects: string[];
}

type ModuleFunction = (input?: unknown) => unknown | Promise<unknown>;

interface LoadedServerModule {
  moduleName: string;
  methods: string[];
  handlers: Map<string, ModuleFunction>;
}

export function subjectForServerModule(app: string, moduleName: string, method: string): string {
  return `bladb.app.${app}.module.${moduleName}.${method}`;
}

export async function discoverServerModules(
  options: DiscoverServerModulesOptions,
): Promise<DiscoveredServerModule[]> {
  const entries = await readdir(options.modulesDir, { withFileTypes: true });
  const files = entries
    .filter((entry) => entry.isFile())
    .filter((entry) => !entry.name.startsWith("_"))
    .filter((entry) => MODULE_EXTENSIONS.has(path.extname(entry.name)))
    .sort((left, right) => left.name.localeCompare(right.name));

  const seen = new Map<string, string>();
  const discovered: DiscoveredServerModule[] = [];

  for (const file of files) {
    const filePath = path.join(options.modulesDir, file.name);
    const moduleName = path.basename(file.name, path.extname(file.name));
    const duplicate = seen.get(moduleName);
    if (duplicate) {
      throw new Error(
        `duplicate server module name \`${moduleName}\` for \`${duplicate}\` and \`${filePath}\``,
      );
    }

    seen.set(moduleName, filePath);
    const loaded = await loadServerModule(filePath, moduleName);
    discovered.push({
      moduleName,
      filePath,
      methods: loaded.methods,
    });
  }

  return discovered;
}

export async function createServerModuleRegistry(
  options: DiscoverServerModulesOptions,
): Promise<ServerModuleRegistry> {
  const entries = await readdir(options.modulesDir, { withFileTypes: true });
  const files = entries
    .filter((entry) => entry.isFile())
    .filter((entry) => !entry.name.startsWith("_"))
    .filter((entry) => MODULE_EXTENSIONS.has(path.extname(entry.name)))
    .sort((left, right) => left.name.localeCompare(right.name));

  const modules = new Map<string, LoadedServerModule>();

  for (const file of files) {
    const filePath = path.join(options.modulesDir, file.name);
    const moduleName = path.basename(file.name, path.extname(file.name));
    if (modules.has(moduleName)) {
      throw new Error(`duplicate server module name \`${moduleName}\``);
    }

    modules.set(moduleName, await loadServerModule(filePath, moduleName));
  }

  return {
    listMethodsForModule(moduleName: string) {
      return modules.get(moduleName)?.methods ?? [];
    },
    async invoke(invocation: ServerModuleInvocation) {
      const loaded = modules.get(invocation.module);
      if (!loaded) {
        throw new Error(`unknown server module \`${invocation.module}\``);
      }

      const handler = loaded.handlers.get(invocation.method);
      if (!handler) {
        throw new Error(
          `server module \`${invocation.module}\` does not export method \`${invocation.method}\``,
        );
      }

      return await withServerModuleDbScope(invocation.db, async () => {
        const previousPayload = Reflect.get(globalThis, "__bladbLauncherPayload");
        Reflect.set(globalThis, "__bladbLauncherPayload", invocation as unknown);
        try {
          return await handler(invocation.input);
        } finally {
          if (previousPayload === undefined) {
            Reflect.deleteProperty(globalThis, "__bladbLauncherPayload");
          } else {
            Reflect.set(globalThis, "__bladbLauncherPayload", previousPayload);
          }
        }
      });
    },
  };
}

export async function createServerModuleLauncher(
  options: CreateServerModuleLauncherOptions,
): Promise<ServerModuleLauncher> {
  const registry = await createServerModuleRegistry({ modulesDir: options.modulesDir });
  const discovered = await discoverServerModules({ modulesDir: options.modulesDir });

  return {
    async start() {
      const subjects: string[] = [];

      for (const moduleEntry of discovered) {
        for (const method of moduleEntry.methods) {
          const subject = subjectForServerModule(options.app, moduleEntry.moduleName, method);
          await options.transport.subscribe(subject, async (payload) => {
            const invocation = payload as ServerModuleInvocation;
            try {
              const data = await registry.invoke(invocation);
              return {
                ok: true,
                data,
                ...(invocation.requestId ? { requestId: invocation.requestId } : {}),
              };
            } catch (error) {
              const message = error instanceof Error ? error.message : "Unknown server module error";
              return {
                ok: false,
                error: {
                  code: "SERVER_MODULE_ERROR",
                  message,
                  module: invocation.module,
                  method: invocation.method,
                  ...(invocation.meta?.traceId ? { traceId: invocation.meta.traceId } : {}),
                },
                ...(invocation.requestId ? { requestId: invocation.requestId } : {}),
              };
            }
          });
          subjects.push(subject);
        }
      }

      return subjects.sort((left, right) => left.localeCompare(right));
    },
  };
}

export async function startServerModules(
  options: StartServerModulesOptions,
): Promise<StartedServerModules> {
  const launcher = await createServerModuleLauncher(options);
  const subjects = await launcher.start();
  return {
    launcher,
    subjects,
  };
}

async function loadServerModule(filePath: string, moduleName: string): Promise<LoadedServerModule> {
  const exportsObject = (await import(pathToFileURL(filePath).href)) as Record<string, unknown>;
  const handlers = new Map<string, ModuleFunction>();

  for (const [exportName, value] of Object.entries(exportsObject)) {
    if (exportName === "default") {
      continue;
    }

    if (typeof value !== "function") {
      continue;
    }

    handlers.set(exportName, value as ModuleFunction);
  }

  return {
    moduleName,
    methods: [...handlers.keys()].sort((left, right) => left.localeCompare(right)),
    handlers,
  };
}
