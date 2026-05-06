import type {
  AuthCommands,
  BrowserAuthModule,
  BrowserSessionStore,
  BrowserUserModule,
  GatewaySession,
  LoginInput,
  RegisterInput,
  UserCommands
} from "@bladb/client";
import {
  createBrowserAuthModule as createManagedAuth,
  createBrowserUserModule as createManagedUser
} from "@bladb/client";
import { useEffect, useRef, useState } from "react";

export interface QueryBehaviorOptions {
  enabled?: boolean;
}

export interface QueryState<T> {
  data: T | null;
  error: Error | null;
  loading: boolean;
  refresh: () => Promise<void>;
}

export function useQuery<T>(
  runner: () => Promise<T>,
  deps: readonly unknown[] = [],
  options: QueryBehaviorOptions = {}
): QueryState<T> {
  const enabled = options.enabled ?? true;
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(enabled);

  const refresh = async () => {
    if (!enabled) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      setData(await runner());
    } catch (caught) {
      setError(caught instanceof Error ? caught : new Error("Unknown query error"));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!enabled) {
      setLoading(false);
      return;
    }

    void refresh();
  }, [...deps, enabled]);

  return { data, error, loading, refresh };
}

export interface MutationState<TArgs extends readonly unknown[], TResult> {
  data: TResult | null;
  error: Error | null;
  loading: boolean;
  run: (...args: TArgs) => Promise<TResult>;
}

export function useMutation<TArgs extends readonly unknown[], TResult>(
  runner: (...args: TArgs) => Promise<TResult>
): MutationState<TArgs, TResult> {
  const [data, setData] = useState<TResult | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(false);

  const run = async (...args: TArgs): Promise<TResult> => {
    setLoading(true);
    setError(null);

    try {
      const result = await runner(...args);
      setData(result);
      return result;
    } catch (caught) {
      const nextError = caught instanceof Error ? caught : new Error("Unknown mutation error");
      setError(nextError);
      throw nextError;
    } finally {
      setLoading(false);
    }
  };

  return { data, error, loading, run };
}

export function useLiveValue<T>(
  runner: () => Promise<T>,
  intervalMs: number,
  deps: readonly unknown[] = [],
  options: QueryBehaviorOptions = {}
): QueryState<T> {
  const timerRef = useRef<number | null>(null);
  const enabled = options.enabled ?? true;
  const query = useQuery(runner, deps, options);

  useEffect(() => {
    if (!enabled) {
      if (timerRef.current !== null) {
        window.clearInterval(timerRef.current);
        timerRef.current = null;
      }
      return;
    }

    if (timerRef.current !== null) {
      window.clearInterval(timerRef.current);
    }

    timerRef.current = window.setInterval(() => {
      void query.refresh();
    }, intervalMs);

    return () => {
      if (timerRef.current !== null) {
        window.clearInterval(timerRef.current);
      }
    };
  }, [...deps, enabled, intervalMs]);

  return query;
}

export interface GatewaySessionState<TSession extends GatewaySession = GatewaySession> {
  session: TSession | null;
  error: Error | null;
  loading: boolean;
  ready: boolean;
  login: (input: LoginInput) => Promise<TSession>;
  register: (input: RegisterInput) => Promise<TSession>;
  refresh: () => Promise<TSession | null>;
  logout: () => void;
}

export interface UserSessionState<TSession extends GatewaySession = GatewaySession>
  extends GatewaySessionState<TSession> {}

function resolveBrowserAuthModule<TSession extends GatewaySession>(
  auth: AuthCommands | BrowserAuthModule<TSession>,
  store?: BrowserSessionStore<TSession>
): BrowserAuthModule<TSession> {
  if (store === undefined) {
    return auth as BrowserAuthModule<TSession>;
  }

  return createManagedAuth(auth, store);
}

function resolveBrowserUserModule<TSession extends GatewaySession>(
  user: UserCommands | BrowserUserModule<TSession>,
  store?: BrowserSessionStore<TSession>
): BrowserUserModule<TSession> {
  if (store === undefined) {
    return user as BrowserUserModule<TSession>;
  }

  return createManagedUser(user, store);
}

export function useGatewaySession<TSession extends GatewaySession>(
  auth: BrowserAuthModule<TSession>
): GatewaySessionState<TSession>;
export function useGatewaySession<TSession extends GatewaySession>(
  auth: AuthCommands,
  store: BrowserSessionStore<TSession>
): GatewaySessionState<TSession>;
export function useGatewaySession<TSession extends GatewaySession>(
  auth: AuthCommands | BrowserAuthModule<TSession>,
  store?: BrowserSessionStore<TSession>
): GatewaySessionState<TSession> {
  const browserAuth = resolveBrowserAuthModule(auth, store);
  const [session, setSession] = useState<TSession | null>(() => browserAuth.read());
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [ready, setReady] = useState<boolean>(false);

  const login = async (input: LoginInput): Promise<TSession> => {
    setLoading(true);
    setError(null);

    try {
      const nextSession = await browserAuth.login(input);
      setSession(nextSession);
      return nextSession;
    } catch (caught) {
      const nextError = caught instanceof Error ? caught : new Error("Unknown login error");
      setError(nextError);
      throw nextError;
    } finally {
      setLoading(false);
      setReady(true);
    }
  };

  const register = async (input: RegisterInput): Promise<TSession> => {
    setLoading(true);
    setError(null);

    try {
      const nextSession = await browserAuth.register(input);
      setSession(nextSession);
      return nextSession;
    } catch (caught) {
      const nextError = caught instanceof Error ? caught : new Error("Unknown register error");
      setError(nextError);
      throw nextError;
    } finally {
      setLoading(false);
      setReady(true);
    }
  };

  const refresh = async (): Promise<TSession | null> => {
    const token = browserAuth.getToken();
    if (!token) {
      setSession(null);
      setReady(true);
      return null;
    }

    setLoading(true);

    try {
      const nextSession = await browserAuth.refresh();
      setSession(nextSession);
      setError(null);
      return nextSession;
    } catch (caught) {
      const nextError = caught instanceof Error ? caught : new Error("Unknown refresh error");
      setError(nextError);
      setSession(null);
      return null;
    } finally {
      setLoading(false);
      setReady(true);
    }
  };

  const logout = () => {
    setError(null);
    browserAuth.logout();
    setSession(null);
    setReady(true);
  };

  useEffect(() => {
    void refresh();
  }, []);

  return {
    session,
    error,
    loading,
    ready,
    login,
    register,
    refresh,
    logout
  };
}

export function useUserSession<TSession extends GatewaySession>(
  user: BrowserUserModule<TSession>
): UserSessionState<TSession>;
export function useUserSession<TSession extends GatewaySession>(
  user: UserCommands,
  store: BrowserSessionStore<TSession>
): UserSessionState<TSession>;
export function useUserSession<TSession extends GatewaySession>(
  user: UserCommands | BrowserUserModule<TSession>,
  store?: BrowserSessionStore<TSession>
): UserSessionState<TSession> {
  const browserUser = resolveBrowserUserModule(user, store);
  return useGatewaySession(browserUser);
}
