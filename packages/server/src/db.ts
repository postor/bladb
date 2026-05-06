import { AsyncLocalStorage } from "node:async_hooks";

const invocationStorage = new AsyncLocalStorage<Record<string, unknown>>();

function missingScopeError(): Error {
  return new Error("db is being accessed outside an active server module invocation");
}

function currentScope(): Record<string, unknown> {
  return invocationStorage.getStore() ?? {};
}

function readScopeValue(pathSegments: string[]): unknown {
  let value: unknown = invocationStorage.getStore();
  if (!value) {
    throw missingScopeError();
  }

  for (const segment of pathSegments) {
    if (typeof value !== "object" || value === null || !(segment in value)) {
      throw new Error(`db.${pathSegments.join(".")} is not available in the active server module scope`);
    }

    value = (value as Record<string, unknown>)[segment];
  }

  return value;
}

function createScopedProxy(pathSegments: string[]): unknown {
  return new Proxy(() => undefined, {
    apply(_target, _thisArg, args) {
      const value = readScopeValue(pathSegments);
      if (typeof value !== "function") {
        throw new Error(`db.${pathSegments.join(".")} is not callable`);
      }

      return value(...args);
    },
    get(_target, property) {
      if (property === Symbol.toStringTag) {
        return "BladbServerDbProxy";
      }

      if (property === "then" && pathSegments.length === 0) {
        return undefined;
      }

      if (typeof property !== "string") {
        return undefined;
      }

      return createScopedProxy([...pathSegments, property]);
    },
  });
}

export const db = createScopedProxy([]) as Record<string, unknown>;

export function withServerModuleDbScope<T>(
  scope: Record<string, unknown> | undefined,
  run: () => Promise<T>,
): Promise<T> {
  return invocationStorage.run(scope ?? {}, run);
}

export function snapshotServerModuleDbScope(): Record<string, unknown> {
  return currentScope();
}
