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
