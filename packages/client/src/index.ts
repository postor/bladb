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

export const key = (
  strings: TemplateStringsArray,
  ...values: SerializedValue[]
): KeyTemplateValue => ({
  __bladb: "key-template",
  parts: [...strings],
  values
});

export interface QueryOptions {
  limit?: number;
  offset?: number;
}

export interface BladbClientOptions {
  baseUrl: string;
  getToken?: () => string | undefined;
  fetcher?: typeof fetch;
}

interface RequestPayload {
  engine: "sql" | "mongo" | "redis";
  action: string;
  [key: string]: unknown;
}

async function post<T>(options: BladbClientOptions, path: string, payload: RequestPayload): Promise<T> {
  const fetcher = options.fetcher ?? fetch;
  const token = options.getToken?.();
  const response = await fetcher(`${options.baseUrl}${path}`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      ...(token ? { authorization: `Bearer ${token}` } : {})
    },
    body: JSON.stringify(payload)
  });

  if (!response.ok) {
    throw new Error(`Bladb request failed: ${response.status} ${response.statusText}`);
  }

  return (await response.json()) as T;
}

export interface MongoQueryBuilder {
  find<T = unknown>(query: Record<string, SerializedValue>, options?: QueryOptions): Promise<T>;
  findOne<T = unknown>(query: Record<string, SerializedValue>): Promise<T>;
  insertOne<T = unknown>(document: Record<string, SerializedValue>): Promise<T>;
}

export interface RedisCommands {
  get<T = unknown>(name: string | KeyTemplateValue): Promise<T>;
  set<T = unknown>(name: string | KeyTemplateValue, value: SerializedValue): Promise<T>;
  incrby<T = unknown>(name: string | KeyTemplateValue, amount: number): Promise<T>;
  decrby<T = unknown>(name: string | KeyTemplateValue, amount: number): Promise<T>;
  publish<T = unknown>(channel: string | KeyTemplateValue, payload: SerializedValue): Promise<T>;
}

export interface BladbClient {
  sql<T = unknown>(strings: TemplateStringsArray, ...values: SerializedValue[]): Promise<T>;
  mongo(collection: string): MongoQueryBuilder;
  redis: RedisCommands;
}

export function createClient(options: BladbClientOptions): BladbClient {
  return {
    sql<T = unknown>(strings: TemplateStringsArray, ...values: SerializedValue[]) {
      const statement = strings.reduce((sql, chunk, index) => {
        if (index >= values.length) {
          return sql + chunk;
        }

        return `${sql}${chunk}?`;
      }, "");

      return post<T>(options, "/query", {
        engine: "sql",
        action: "query",
        statement,
        values: values.map((value) => serialize(value))
      });
    },

    mongo(collection: string): MongoQueryBuilder {
      return {
        find<T = unknown>(query: Record<string, SerializedValue>, queryOptions?: QueryOptions) {
          return post<T>(options, "/query", {
            engine: "mongo",
            action: "find",
            collection,
            query: serialize(query),
            options: queryOptions
          });
        },

        findOne<T = unknown>(query: Record<string, SerializedValue>) {
          return post<T>(options, "/query", {
            engine: "mongo",
            action: "findOne",
            collection,
            query: serialize(query)
          });
        },

        insertOne<T = unknown>(document: Record<string, SerializedValue>) {
          return post<T>(options, "/query", {
            engine: "mongo",
            action: "insertOne",
            collection,
            document: serialize(document)
          });
        }
      };
    },

    redis: {
      get<T = unknown>(name: string | KeyTemplateValue) {
        return post<T>(options, "/query", {
          engine: "redis",
          action: "get",
          name: typeof name === "string" ? name : serialize(name)
        });
      },

      set<T = unknown>(name: string | KeyTemplateValue, value: SerializedValue) {
        return post<T>(options, "/query", {
          engine: "redis",
          action: "set",
          name: typeof name === "string" ? name : serialize(name),
          value: serialize(value)
        });
      },

      incrby<T = unknown>(name: string | KeyTemplateValue, amount: number) {
        return post<T>(options, "/query", {
          engine: "redis",
          action: "incrby",
          name: typeof name === "string" ? name : serialize(name),
          amount
        });
      },

      decrby<T = unknown>(name: string | KeyTemplateValue, amount: number) {
        return post<T>(options, "/query", {
          engine: "redis",
          action: "decrby",
          name: typeof name === "string" ? name : serialize(name),
          amount
        });
      },

      publish<T = unknown>(channel: string | KeyTemplateValue, payload: SerializedValue) {
        return post<T>(options, "/query", {
          engine: "redis",
          action: "publish",
          channel: typeof channel === "string" ? channel : serialize(channel),
          payload: serialize(payload)
        });
      }
    }
  };
}
