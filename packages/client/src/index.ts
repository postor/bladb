export type Primitive = string | number | boolean | null;

export interface ContextValue {
  readonly __bladb: "ctx";
  readonly key: string;
  readonly token: string;
}

export interface KeyTemplateValue {
  readonly __bladb: "key-template";
  readonly parts: readonly string[];
  readonly values: readonly SerializedValue[];
}

export type SerializedValue =
  | Primitive
  | ContextValue
  | KeyTemplateValue
  | SerializedValue[]
  | { [key: string]: SerializedValue };

export interface RequestMetaInput {
  resource?: string;
  policy?: string;
  traceId?: string;
  params?: Record<string, SerializedValue>;
}

export interface SerializedRequestMeta {
  resource?: string;
  policy?: string;
  traceId?: string;
  params?: Record<string, unknown>;
}

const createContextValue = (token: string, key: string): ContextValue =>
  Object.freeze({
    __bladb: "ctx",
    key,
    token
  });

export const UID = createContextValue("UID", "uid");
export const TENANT_ID = createContextValue("TENANT_ID", "tenantId");
export const ROLES = createContextValue("ROLES", "roles");
export const PERMISSION_VERSION = createContextValue("PERMISSION_VERSION", "permissionVersion");

export const RESERVED_CLAIMS = Object.freeze({
  UID: UID.key,
  TENANT_ID: TENANT_ID.key,
  ROLES: ROLES.key,
  PERMISSION_VERSION: PERMISSION_VERSION.key
});

const isContextValue = (value: unknown): value is ContextValue =>
  typeof value === "object" &&
  value !== null &&
  (value as ContextValue).__bladb === "ctx";

const isKeyTemplateValue = (value: unknown): value is KeyTemplateValue =>
  typeof value === "object" &&
  value !== null &&
  (value as KeyTemplateValue).__bladb === "key-template";

const serialize = (value: SerializedValue): unknown => {
  if (isContextValue(value)) {
    return { $ctx: value.key, token: value.token };
  }

  if (isKeyTemplateValue(value)) {
    return {
      $template: "key",
      parts: [...value.parts],
      values: value.values.map((entry) => serialize(entry))
    };
  }

  if (Array.isArray(value)) {
    return value.map((entry) => serialize(entry));
  }

  if (typeof value === "object" && value !== null) {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [key, serialize(entry as SerializedValue)])
    );
  }

  return value;
};

const serializeMeta = (meta?: RequestMetaInput): SerializedRequestMeta => ({
  ...(meta?.resource ? { resource: meta.resource } : {}),
  ...(meta?.policy ? { policy: meta.policy } : {}),
  ...(meta?.traceId ? { traceId: meta.traceId } : {}),
  ...(meta?.params
    ? {
        params: Object.fromEntries(
          Object.entries(meta.params).map(([key, value]) => [key, serialize(value)])
        )
      }
    : {})
});

const mergeMeta = (
  baseMeta?: RequestMetaInput,
  requestMeta?: RequestMetaInput
): RequestMetaInput | undefined => {
  if (!baseMeta && !requestMeta) {
    return undefined;
  }

  return {
    ...baseMeta,
    ...requestMeta,
    params: {
      ...(baseMeta?.params ?? {}),
      ...(requestMeta?.params ?? {})
    }
  };
};

export const key = (
  strings: TemplateStringsArray,
  ...values: SerializedValue[]
): KeyTemplateValue => ({
  __bladb: "key-template",
  parts: [...strings],
  values
});

export const template = key;

export interface QueryOptions {
  limit?: number;
  offset?: number;
}

export interface BladbClientOptions {
  baseUrl: string;
  getToken?: () => string | undefined;
  fetcher?: typeof fetch;
}

export interface GatewaySessionUser {
  app: string;
  uid: string;
  tenantId: string;
  email: string;
  displayName: string;
  roles: string[];
}

export interface GatewaySession<TUser = GatewaySessionUser> {
  token: string;
  user: TUser;
}

export interface BrowserSessionStore<TSession = GatewaySession> {
  read(): TSession | null;
  persist(session: TSession): void;
  clear(): void;
  getToken(): string | undefined;
}

export interface BrowserSessionStoreOptions {
  tokenKey: string;
  sessionKey: string;
}

export interface BrowserAppModuleOptions<
  TSession extends GatewaySession,
  TDefinitions extends Record<string, AppRouteDefinition>
> extends BrowserSessionStoreOptions {
  baseUrl: string;
  appName: string;
  routes: TDefinitions;
  fetcher?: typeof fetch;
}

export interface BrowserAppModule<
  TSession extends GatewaySession,
  TDefinitions extends Record<string, AppRouteDefinition>
> {
  db: BladbClient;
  sessionStore: BrowserSessionStore<TSession>;
  auth: BrowserAuthModule<TSession>;
  api: TypedAppClient<TDefinitions>;
}

export interface LoginInput {
  app: string;
  email: string;
  password: string;
}

export interface RegisterInput extends LoginInput {
  displayName: string;
}

export function createBrowserSessionStore<TSession extends { token: string } = GatewaySession>(
  options: BrowserSessionStoreOptions
): BrowserSessionStore<TSession> {
  return {
    read(): TSession | null {
      const raw = window.localStorage.getItem(options.sessionKey);
      if (!raw) {
        return null;
      }

      try {
        return JSON.parse(raw) as TSession;
      } catch {
        this.clear();
        return null;
      }
    },

    persist(session: TSession) {
      window.localStorage.setItem(options.tokenKey, session.token);
      window.localStorage.setItem(options.sessionKey, JSON.stringify(session));
    },

    clear() {
      window.localStorage.removeItem(options.tokenKey);
      window.localStorage.removeItem(options.sessionKey);
    },

    getToken() {
      return window.localStorage.getItem(options.tokenKey) ?? undefined;
    }
  };
}

export class BladbError extends Error {
  readonly code?: string;
  readonly status: number;
  readonly traceId?: string;

  constructor(message: string, options: { status: number; code?: string; traceId?: string }) {
    super(message);
    this.name = "BladbError";
    this.status = options.status;
    this.code = options.code;
    this.traceId = options.traceId;
  }
}

type Engine = "sql" | "mongo" | "redis" | "mqtt" | "kafka" | "mq";
type Kind = "query" | "command" | "stream" | "queue";

interface RequestPayload {
  kind: Kind;
  engine: Engine;
  action: string;
  meta?: SerializedRequestMeta;
  [key: string]: unknown;
}

interface ErrorPayload {
  ok?: boolean;
  code?: string;
  message?: string;
  meta?: {
    traceId?: string;
  };
}

interface JsonRequestOptions {
  path: string;
  method?: "GET" | "POST";
  body?: unknown;
  auth?: "optional" | "required" | "none";
}

async function requestJson<T>(
  options: BladbClientOptions,
  requestOptions: JsonRequestOptions
): Promise<T> {
  const fetcher = options.fetcher ?? fetch;
  const token = requestOptions.auth === "none" ? undefined : options.getToken?.();
  const response = await fetcher(`${options.baseUrl}${requestOptions.path}`, {
    method: requestOptions.method ?? "GET",
    headers: {
      ...(requestOptions.body === undefined ? {} : { "content-type": "application/json" }),
      ...(token ? { authorization: `Bearer ${token}` } : {})
    },
    ...(requestOptions.body === undefined ? {} : { body: JSON.stringify(requestOptions.body) })
  });

  if (!response.ok) {
    let errorPayload: ErrorPayload | undefined;
    try {
      errorPayload = (await response.json()) as ErrorPayload;
    } catch {
      errorPayload = undefined;
    }

    throw new BladbError(errorPayload?.message ?? response.statusText, {
      status: response.status,
      code: errorPayload?.code,
      traceId: errorPayload?.meta?.traceId
    });
  }

  const body = (await response.json()) as { data?: T };
  return (body.data ?? (body as T)) as T;
}

async function post<T>(options: BladbClientOptions, payload: RequestPayload): Promise<T> {
  return await requestJson<T>(options, {
    path: "/execute",
    method: "POST",
    body: payload,
    auth: "optional"
  });
}

export interface MongoQueryBuilder {
  find<T = unknown>(
    query: Record<string, SerializedValue>,
    options?: QueryOptions,
    meta?: RequestMetaInput
  ): Promise<T>;
  findOne<T = unknown>(query: Record<string, SerializedValue>, meta?: RequestMetaInput): Promise<T>;
  insertOne<T = unknown>(
    document: Record<string, SerializedValue>,
    meta?: RequestMetaInput
  ): Promise<T>;
}

export interface RedisCommands {
  get<T = unknown>(name: string | KeyTemplateValue, meta?: RequestMetaInput): Promise<T>;
  set<T = unknown>(
    name: string | KeyTemplateValue,
    value: SerializedValue,
    meta?: RequestMetaInput
  ): Promise<T>;
  incrby<T = unknown>(name: string | KeyTemplateValue, amount: number, meta?: RequestMetaInput): Promise<T>;
  decrby<T = unknown>(name: string | KeyTemplateValue, amount: number, meta?: RequestMetaInput): Promise<T>;
  publish<T = unknown>(
    channel: string | KeyTemplateValue,
    payload: SerializedValue,
    meta?: RequestMetaInput
  ): Promise<T>;
}

export interface MqttCommands {
  publish<T = unknown>(
    topic: string | KeyTemplateValue,
    payload: SerializedValue,
    meta?: RequestMetaInput
  ): Promise<T>;
}

export interface KafkaCommands {
  produce<T = unknown>(topic: string, payload: SerializedValue, meta?: RequestMetaInput): Promise<T>;
}

export interface QueueCommands {
  publish<T = unknown>(queue: string, message: SerializedValue, meta?: RequestMetaInput): Promise<T>;
  publishDelayed<T = unknown>(
    queue: string,
    message: SerializedValue,
    delayMs: number,
    meta?: RequestMetaInput
  ): Promise<T>;
}

export interface AuthCommands {
  login<TSession = GatewaySession>(input: LoginInput): Promise<TSession>;
  register<TSession = GatewaySession>(input: RegisterInput): Promise<TSession>;
  me<TSession = GatewaySession>(): Promise<TSession>;
}

export interface BrowserAuthModule<TSession extends GatewaySession = GatewaySession>
  extends AuthCommands {
  read(): TSession | null;
  getToken(): string | undefined;
  login(input: LoginInput): Promise<TSession>;
  register(input: RegisterInput): Promise<TSession>;
  refresh(): Promise<TSession | null>;
  logout(): void;
}

export interface AppEndpointClient {
  get<T = unknown>(path?: string): Promise<T>;
  post<T = unknown>(path: string, body?: Record<string, unknown> | SerializedValue): Promise<T>;
  stream<T = unknown>(
    path: string,
    options: { signal?: AbortSignal; onMessage: (payload: T) => void }
  ): Promise<void>;
}

export interface AppGetRouteDefinition<TArgs extends readonly unknown[], TResponse> {
  readonly method: "GET";
  readonly resolvePath: (...args: TArgs) => string;
}

export interface AppPostRouteDefinition<
  TArgs extends readonly unknown[],
  TBody,
  TResponse
> {
  readonly method: "POST";
  readonly resolvePath: (...args: TArgs) => string;
  readonly resolveBody: (...args: TArgs) => TBody;
}

export type AppRouteDefinition =
  | AppGetRouteDefinition<readonly unknown[], unknown>
  | AppPostRouteDefinition<readonly unknown[], unknown, unknown>;

export type TypedAppClient<TDefinitions extends Record<string, AppRouteDefinition>> = {
  [TKey in keyof TDefinitions]:
    TDefinitions[TKey] extends AppGetRouteDefinition<infer TArgs, infer TResponse>
      ? (...args: TArgs) => Promise<TResponse>
      : TDefinitions[TKey] extends AppPostRouteDefinition<infer TArgs, infer _TBody, infer TResponse>
        ? (...args: TArgs) => Promise<TResponse>
        : never;
};

export function appGet<TResponse>(
  path: string
): AppGetRouteDefinition<[], TResponse>;
export function appGet<TArgs extends readonly unknown[], TResponse>(
  path: (...args: TArgs) => string
): AppGetRouteDefinition<TArgs, TResponse>;
export function appGet<TArgs extends readonly unknown[], TResponse>(
  path: string | ((...args: TArgs) => string)
): AppGetRouteDefinition<TArgs, TResponse> {
  return {
    method: "GET",
    resolvePath:
      typeof path === "string"
        ? (() => path) as (...args: TArgs) => string
        : path
  };
}

export function appPost<TBody, TResponse>(
  path: string
): AppPostRouteDefinition<[TBody], TBody, TResponse>;
export function appPost<TArgs extends readonly unknown[], TBody, TResponse>(
  path: (...args: TArgs) => string,
  body: (...args: TArgs) => TBody
): AppPostRouteDefinition<TArgs, TBody, TResponse>;
export function appPost<TArgs extends readonly unknown[], TBody, TResponse>(
  path: string | ((...args: TArgs) => string),
  body?: (...args: TArgs) => TBody
): AppPostRouteDefinition<TArgs, TBody, TResponse> {
  const resolvePath =
    typeof path === "string"
      ? (() => path) as (...args: TArgs) => string
      : path;
  const resolveBody =
    body ?? ((...args: TArgs) => args[0] as TBody);

  return {
    method: "POST",
    resolvePath,
    resolveBody
  };
}

export function createTypedAppClient<TDefinitions extends Record<string, AppRouteDefinition>>(
  client: AppEndpointClient,
  definitions: TDefinitions
): TypedAppClient<TDefinitions> {
  const entries = Object.entries(definitions).map(([name, definition]) => {
    if (definition.method === "GET") {
      return [
        name,
        (...args: readonly unknown[]) =>
          client.get(definition.resolvePath(...args))
      ];
    }

    return [
      name,
      (...args: readonly unknown[]) =>
        client.post(
          definition.resolvePath(...args),
          definition.resolveBody(...args)
        )
    ];
  });

  return Object.fromEntries(entries) as TypedAppClient<TDefinitions>;
}

export function createBrowserAppModule<
  TSession extends GatewaySession = GatewaySession,
  TDefinitions extends Record<string, AppRouteDefinition>
>(
  options: BrowserAppModuleOptions<TSession, TDefinitions>
): BrowserAppModule<TSession, TDefinitions> {
  const sessionStore = createBrowserSessionStore<TSession>({
    tokenKey: options.tokenKey,
    sessionKey: options.sessionKey
  });
  const db = createClient({
    baseUrl: options.baseUrl,
    fetcher: options.fetcher,
    getToken: () => sessionStore.getToken()
  });
  const auth = createBrowserAuthModule<TSession>(db.auth, sessionStore);

  return {
    db,
    sessionStore,
    auth,
    api: createTypedAppClient(db.app(options.appName), options.routes)
  };
}

export function createBrowserAuthModule<TSession extends GatewaySession = GatewaySession>(
  auth: AuthCommands,
  store: BrowserSessionStore<TSession>
): BrowserAuthModule<TSession> {
  const applySession = (session: TSession | null) => {
    if (session) {
      store.persist(session);
      return session;
    }

    store.clear();
    return null;
  };

  return {
    read() {
      return store.read();
    },

    getToken() {
      return store.getToken();
    },

    async login(input: LoginInput) {
      const session = await auth.login<TSession>(input);
      return applySession(session) as TSession;
    },

    async register(input: RegisterInput) {
      const session = await auth.register<TSession>(input);
      return applySession(session) as TSession;
    },

    async refresh() {
      if (!store.getToken()) {
        return applySession(null);
      }

      try {
        const session = await auth.me<TSession>();
        return applySession(session);
      } catch {
        return applySession(null);
      }
    },

    logout() {
      applySession(null);
    },

    me<TCurrentSession = GatewaySession>() {
      return auth.me<TCurrentSession>();
    }
  };
}

export interface BladbClient {
  withMeta(meta: RequestMetaInput): BladbClient;
  app(name: string): AppEndpointClient;
  auth: AuthCommands;
  sql<T = unknown>(strings: TemplateStringsArray, ...values: SerializedValue[]): Promise<T>;
  mongo(collection: string): MongoQueryBuilder;
  redis: RedisCommands;
  mqtt: MqttCommands;
  kafka: KafkaCommands;
  mq: QueueCommands;
}

function buildClient(options: BladbClientOptions, baseMeta?: RequestMetaInput): BladbClient {
  const classifySql = (statement: string): { kind: Kind; action: string } => {
    const verb = statement.trimStart().split(/\s+/, 1)[0]?.toLowerCase();

    switch (verb) {
      case "select":
        return { kind: "query", action: "select" };
      case "insert":
      case "update":
      case "delete":
        return { kind: "command", action: verb };
      default:
        return { kind: "query", action: "select" };
    }
  };

  const request = <T>(
    payload: Omit<RequestPayload, "meta"> & { meta?: RequestMetaInput }
  ): Promise<T> =>
    post<T>(options, {
      ...payload,
      meta: serializeMeta(mergeMeta(baseMeta, payload.meta))
    });

  return {
    withMeta(meta: RequestMetaInput): BladbClient {
      return buildClient(options, mergeMeta(baseMeta, meta));
    },

    app(name: string): AppEndpointClient {
      const normalizedName = name.replace(/^\/+|\/+$/g, "");
      const basePath = `/apps/${normalizedName}`;

      return {
        get<T = unknown>(path = "") {
          const suffix = path.replace(/^\/+/, "");
          return requestJson<T>(options, {
            path: suffix ? `${basePath}/${suffix}` : basePath,
            method: "GET",
            auth: "required"
          });
        },

        post<T = unknown>(path: string, body?: Record<string, unknown> | SerializedValue) {
          const suffix = path.replace(/^\/+/, "");
          return requestJson<T>(options, {
            path: suffix ? `${basePath}/${suffix}` : basePath,
            method: "POST",
            body,
            auth: "required"
          });
        },

        async stream<T = unknown>(
          path: string,
          streamOptions: { signal?: AbortSignal; onMessage: (payload: T) => void }
        ) {
          const suffix = path.replace(/^\/+/, "");
          const token = options.getToken?.();
          if (!token) {
            throw new BladbError("missing bearer token", {
              status: 401,
              code: "AUTH_EXPIRED"
            });
          }

          const response = await (options.fetcher ?? fetch)(
            `${options.baseUrl}${suffix ? `${basePath}/${suffix}` : basePath}`,
            {
              method: "GET",
              headers: {
                authorization: `Bearer ${token}`,
                accept: "text/event-stream"
              },
              signal: streamOptions.signal
            }
          );

          if (!response.ok) {
            throw new BladbError(response.statusText, {
              status: response.status
            });
          }

          const reader = response.body?.getReader();
          if (!reader) {
            throw new BladbError("stream body is unavailable", {
              status: 500
            });
          }

          const decoder = new TextDecoder();
          let buffered = "";

          while (true) {
            const { done, value } = await reader.read();
            if (done) {
              break;
            }

            buffered += decoder.decode(value, { stream: true });
            let boundary = buffered.indexOf("\n\n");
            while (boundary >= 0) {
              const frame = buffered.slice(0, boundary);
              buffered = buffered.slice(boundary + 2);
              const dataLines = frame
                .split("\n")
                .filter((line) => line.startsWith("data: "))
                .map((line) => line.slice(6));
              if (dataLines.length > 0) {
                streamOptions.onMessage(JSON.parse(dataLines.join("\n")) as T);
              }
              boundary = buffered.indexOf("\n\n");
            }
          }
        }
      };
    },

    auth: {
      login<TSession = GatewaySession>(input: LoginInput) {
        return requestJson<TSession>(options, {
          path: "/auth/login",
          method: "POST",
          body: input,
          auth: "none"
        });
      },

      register<TSession = GatewaySession>(input: RegisterInput) {
        return requestJson<TSession>(options, {
          path: "/auth/register",
          method: "POST",
          body: input,
          auth: "none"
        });
      },

      me<TSession = GatewaySession>() {
        return requestJson<TSession>(options, {
          path: "/auth/me",
          method: "GET",
          auth: "required"
        });
      }
    },

    sql<T = unknown>(strings: TemplateStringsArray, ...values: SerializedValue[]) {
      const statement = strings.reduce((sql, chunk, index) => {
        if (index >= values.length) {
          return sql + chunk;
        }

        return `${sql}${chunk}?`;
      }, "");

      const sqlRequest = classifySql(statement);

      return request<T>({
        kind: sqlRequest.kind,
        engine: "sql",
        action: sqlRequest.action,
        statement,
        values: values.map((value) => serialize(value))
      });
    },

    mongo(collection: string): MongoQueryBuilder {
      const collectionMeta =
        baseMeta?.resource === undefined ? { ...baseMeta, resource: collection } : baseMeta;

      return {
        find<T = unknown>(
          query: Record<string, SerializedValue>,
          queryOptions?: QueryOptions,
          meta?: RequestMetaInput
        ) {
          return post<T>(options, {
            kind: "query",
            engine: "mongo",
            action: "find",
            meta: serializeMeta(mergeMeta(collectionMeta, meta)),
            collection,
            query: serialize(query),
            options: queryOptions
          });
        },

        findOne<T = unknown>(query: Record<string, SerializedValue>, meta?: RequestMetaInput) {
          return post<T>(options, {
            kind: "query",
            engine: "mongo",
            action: "findOne",
            meta: serializeMeta(mergeMeta(collectionMeta, meta)),
            collection,
            query: serialize(query)
          });
        },

        insertOne<T = unknown>(document: Record<string, SerializedValue>, meta?: RequestMetaInput) {
          return post<T>(options, {
            kind: "command",
            engine: "mongo",
            action: "insertOne",
            meta: serializeMeta(mergeMeta(collectionMeta, meta)),
            collection,
            document: serialize(document)
          });
        }
      };
    },

    redis: {
      get<T = unknown>(name: string | KeyTemplateValue, meta?: RequestMetaInput) {
        return request<T>({
          kind: "query",
          engine: "redis",
          action: "get",
          meta,
          name: typeof name === "string" ? name : serialize(name)
        });
      },

      set<T = unknown>(name: string | KeyTemplateValue, value: SerializedValue, meta?: RequestMetaInput) {
        return request<T>({
          kind: "command",
          engine: "redis",
          action: "set",
          meta,
          name: typeof name === "string" ? name : serialize(name),
          value: serialize(value)
        });
      },

      incrby<T = unknown>(name: string | KeyTemplateValue, amount: number, meta?: RequestMetaInput) {
        return request<T>({
          kind: "command",
          engine: "redis",
          action: "incrby",
          meta,
          name: typeof name === "string" ? name : serialize(name),
          amount
        });
      },

      decrby<T = unknown>(name: string | KeyTemplateValue, amount: number, meta?: RequestMetaInput) {
        return request<T>({
          kind: "command",
          engine: "redis",
          action: "decrby",
          meta,
          name: typeof name === "string" ? name : serialize(name),
          amount
        });
      },

      publish<T = unknown>(
        channel: string | KeyTemplateValue,
        payload: SerializedValue,
        meta?: RequestMetaInput
      ) {
        return request<T>({
          kind: "stream",
          engine: "redis",
          action: "publish",
          meta,
          channel: typeof channel === "string" ? channel : serialize(channel),
          payload: serialize(payload)
        });
      }
    },

    mqtt: {
      publish<T = unknown>(
        topic: string | KeyTemplateValue,
        payload: SerializedValue,
        meta?: RequestMetaInput
      ) {
        return request<T>({
          kind: "stream",
          engine: "mqtt",
          action: "publish",
          meta,
          topic: typeof topic === "string" ? topic : serialize(topic),
          payload: serialize(payload)
        });
      }
    },

    kafka: {
      produce<T = unknown>(topic: string, payload: SerializedValue, meta?: RequestMetaInput) {
        return request<T>({
          kind: "stream",
          engine: "kafka",
          action: "produce",
          meta,
          topic,
          payload: serialize(payload)
        });
      }
    },

    mq: {
      publish<T = unknown>(queue: string, message: SerializedValue, meta?: RequestMetaInput) {
        return request<T>({
          kind: "queue",
          engine: "mq",
          action: "publish",
          meta,
          queue,
          message: serialize(message)
        });
      },

      publishDelayed<T = unknown>(
        queue: string,
        message: SerializedValue,
        delayMs: number,
        meta?: RequestMetaInput
      ) {
        return request<T>({
          kind: "queue",
          engine: "mq",
          action: "publishDelayed",
          meta,
          queue,
          message: serialize(message),
          delayMs
        });
      }
    }
  };
}

export function createClient(options: BladbClientOptions): BladbClient {
  return buildClient(options);
}
