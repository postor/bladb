import type {
  AuthCommands,
  BrowserAuthModule,
  BrowserSessionStore,
  GatewaySession,
  LoginInput,
  RegisterInput
} from "@bladb/client";
import { createBrowserAuthModule as createManagedAuth } from "@bladb/client";
import { useEffect, useRef, useState } from "react";

export interface QueryState<T> {
  data: T | null;
  error: Error | null;
  loading: boolean;
  refresh: () => Promise<void>;
}

export function useQuery<T>(runner: () => Promise<T>, deps: readonly unknown[] = []): QueryState<T> {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = async () => {
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
    void refresh();
  }, deps);

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
  deps: readonly unknown[] = []
): QueryState<T> {
  const timerRef = useRef<number | null>(null);
  const query = useQuery(runner, deps);

  useEffect(() => {
    void query.refresh();

    timerRef.current = window.setInterval(() => {
      void query.refresh();
    }, intervalMs);

    return () => {
      if (timerRef.current !== null) {
        window.clearInterval(timerRef.current);
      }
    };
  }, deps);

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

function resolveBrowserAuthModule<TSession extends GatewaySession>(
  auth: AuthCommands | BrowserAuthModule<TSession>,
  store?: BrowserSessionStore<TSession>
): BrowserAuthModule<TSession> {
  if (store === undefined) {
    return auth as BrowserAuthModule<TSession>;
  }

  return createManagedAuth(auth, store);
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
  const [loading, setLoading] = useState<boolean>(() => Boolean(browserAuth.getToken()));
  const [ready, setReady] = useState<boolean>(() => !browserAuth.getToken());

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
    if (!browserAuth.getToken()) {
      setReady(true);
      return;
    }

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
