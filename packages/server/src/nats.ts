import type { ServerModuleTransport } from "./launcher.ts";

export interface NatsServerModuleTransportOptions {
  servers: string | string[];
  connectFn?: (options: { servers: string | string[] }) => Promise<NatsConnectionLike>;
}

export interface NatsServerModuleTransport extends ServerModuleTransport {
  drain(): Promise<void>;
}

export interface NatsMessageLike {
  data: Uint8Array;
  respond(data: Uint8Array): void | Promise<void>;
}

export interface NatsSubscriptionLike extends AsyncIterable<NatsMessageLike> {}

export interface NatsConnectionLike {
  subscribe(subject: string): NatsSubscriptionLike;
  drain(): Promise<void>;
}

interface JsonCodecLike {
  encode(value: unknown): Uint8Array;
  decode(data: Uint8Array): unknown;
}

export async function createNatsServerModuleTransport(
  options: NatsServerModuleTransportOptions,
): Promise<NatsServerModuleTransport> {
  const connection = options.connectFn
    ? await options.connectFn({ servers: options.servers })
    : await connectWithNatsPackage(options.servers);
  const codec = await createJsonCodec();
  const loops: Promise<void>[] = [];

  return {
    async subscribe(subject, handler) {
      const subscription = connection.subscribe(subject);
      const loop = startSubscriptionLoop(subscription, codec, handler);
      loops.push(loop);
    },
    async drain() {
      await connection.drain();
      await Promise.allSettled(loops);
    },
  };
}

async function connectWithNatsPackage(servers: string | string[]): Promise<NatsConnectionLike> {
  const { connect } = await import("nats");
  return await connect({ servers });
}

async function createJsonCodec(): Promise<JsonCodecLike> {
  const { JSONCodec } = await import("nats");
  return JSONCodec();
}

async function startSubscriptionLoop(
  subscription: NatsSubscriptionLike,
  codec: JsonCodecLike,
  handler: (payload: unknown) => Promise<unknown>,
): Promise<void> {
  for await (const message of subscription) {
    const payload = codec.decode(message.data);
    const response = await handler(payload);
    await message.respond(codec.encode(response));
  }
}
