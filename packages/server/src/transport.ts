import type { ServerModuleTransport } from "./launcher.ts";

export interface InMemoryServerModuleTransport extends ServerModuleTransport {
  listSubjects(): string[];
  request(subject: string, payload: unknown): Promise<unknown>;
}

export function createInMemoryServerModuleTransport(): InMemoryServerModuleTransport {
  const handlers = new Map<string, (payload: unknown) => Promise<unknown>>();

  return {
    async subscribe(subject: string, handler: (payload: unknown) => Promise<unknown>) {
      handlers.set(subject, handler);
    },
    listSubjects() {
      return [...handlers.keys()].sort((left, right) => left.localeCompare(right));
    },
    async request(subject: string, payload: unknown) {
      const handler = handlers.get(subject);
      if (!handler) {
        throw new Error(`missing handler for ${subject}`);
      }

      return await handler(payload);
    },
  };
}
