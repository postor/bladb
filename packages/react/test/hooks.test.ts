import assert from "node:assert/strict";
import test from "node:test";
import {
  createBrowserSessionStore,
  createBrowserUserModule,
  type GatewaySession,
  type UserCommands
} from "../../client/src/index.ts";
import { JSDOM } from "jsdom";
import React, { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { useLiveValue, useQuery, useUserSession } from "../src/index.ts";

type Deferred<T> = {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (error: unknown) => void;
};

function createDeferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });

  return { promise, resolve, reject };
}

function installDom() {
  const dom = new JSDOM("<!doctype html><html><body><div id=\"root\"></div></body></html>", {
    url: "http://localhost/"
  });

  Object.defineProperty(globalThis, "IS_REACT_ACT_ENVIRONMENT", {
    configurable: true,
    value: true
  });

  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: dom.window
  });
  Object.defineProperty(globalThis, "document", {
    configurable: true,
    value: dom.window.document
  });
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: dom.window.navigator
  });
  Object.defineProperty(globalThis, "HTMLElement", {
    configurable: true,
    value: dom.window.HTMLElement
  });

  return {
    dom,
    cleanup() {
      dom.window.close();
      delete (globalThis as Partial<typeof globalThis>).window;
      delete (globalThis as Partial<typeof globalThis>).document;
      delete (globalThis as Partial<typeof globalThis>).navigator;
      delete (globalThis as Partial<typeof globalThis>).HTMLElement;
      delete (globalThis as Partial<typeof globalThis>).IS_REACT_ACT_ENVIRONMENT;
    }
  };
}

async function flushEffects() {
  await act(async () => {
    await Promise.resolve();
  });
}

test("useLiveValue runs only one initial refresh when enabled", async () => {
  const { cleanup } = installDom();
  const container = document.getElementById("root");
  assert.ok(container);
  const root = createRoot(container);
  const firstCall = createDeferred<number>();
  let callCount = 0;

  function Harness() {
    useLiveValue(
      async () => {
        callCount += 1;
        return await firstCall.promise;
      },
      60_000,
      [],
      { enabled: true }
    );
    return null;
  }

  try {
    await act(async () => {
      root.render(createElement(Harness));
    });

    assert.equal(callCount, 1);

    firstCall.resolve(1);
    await flushEffects();
  } finally {
    await act(async () => {
      root.unmount();
    });
    cleanup();
  }
});

test("useLiveValue stays idle while disabled and starts once after enabling", async () => {
  const { cleanup } = installDom();
  const container = document.getElementById("root");
  assert.ok(container);
  const root = createRoot(container);
  const firstCall = createDeferred<number>();
  let callCount = 0;
  let enabled = false;
  let rerender: (() => Promise<void>) | null = null;

  function Harness() {
    useLiveValue(
      async () => {
        callCount += 1;
        return await firstCall.promise;
      },
      60_000,
      [enabled],
      { enabled }
    );
    rerender = async () => {
      await act(async () => {
        root.render(createElement(Harness));
      });
    };
    return null;
  }

  try {
    await act(async () => {
      root.render(createElement(Harness));
    });
    assert.equal(callCount, 0);

    enabled = true;
    await rerender?.();
    assert.equal(callCount, 1);

    firstCall.resolve(1);
    await flushEffects();
  } finally {
    await act(async () => {
      root.unmount();
    });
    cleanup();
  }
});

test("useQuery does not refresh while disabled", async () => {
  const { cleanup } = installDom();
  const container = document.getElementById("root");
  assert.ok(container);
  const root = createRoot(container);
  let callCount = 0;
  let snapshot: { loading: boolean; data: number | null } | null = null;

  function Harness() {
    const query = useQuery(
      async () => {
        callCount += 1;
        return 7;
      },
      [],
      { enabled: false }
    );
    snapshot = {
      loading: query.loading,
      data: query.data
    };
    return null;
  }

  try {
    await act(async () => {
      root.render(createElement(Harness));
    });

    assert.equal(callCount, 0);
    assert.deepEqual(snapshot, {
      loading: false,
      data: null
    });
  } finally {
    await act(async () => {
      root.unmount();
    });
    cleanup();
  }
});

test("useUserSession works directly with the browser-managed db.user module", async () => {
  const { cleanup } = installDom();
  const container = document.getElementById("root");
  assert.ok(container);
  const root = createRoot(container);

  const existingSession: GatewaySession = {
    token: "token_flash_sale",
    user: {
      app: "flash-sale",
      uid: "buyer_1",
      tenantId: "tenant_flash",
      email: "buyer@flash-sale.demo",
      displayName: "Buyer One",
      roles: ["buyer"]
    }
  };

  const sessionStore = createBrowserSessionStore<GatewaySession>({
    tokenKey: "flash-sale.user.token",
    sessionKey: "flash-sale.user.session"
  });
  sessionStore.persist(existingSession);

  let meCalls = 0;
  const user = createBrowserUserModule(
    {
      login: async () => existingSession,
      register: async () => existingSession,
      me: async () => {
        meCalls += 1;
        return existingSession;
      }
    },
    sessionStore
  );

  let snapshot: ReturnType<typeof useUserSession<GatewaySession>> | null = null;

  function Harness() {
    snapshot = useUserSession(user);
    return null;
  }

  try {
    await act(async () => {
      root.render(createElement(Harness));
    });

    await flushEffects();

    assert.ok(snapshot);
    assert.deepEqual(snapshot.session, existingSession);
    assert.equal(snapshot.ready, true);
    assert.equal(snapshot.loading, false);
    assert.equal(meCalls, 1);

    await act(async () => {
      snapshot?.logout();
    });

    assert.equal(snapshot?.session, null);
    assert.equal(sessionStore.read(), null);
    assert.equal(window.localStorage.getItem("flash-sale.user.token"), null);
  } finally {
    await act(async () => {
      root.unmount();
    });
    cleanup();
  }
});

test("useUserSession can layer session state on top of plain db.user commands", async () => {
  const { cleanup } = installDom();
  const container = document.getElementById("root");
  assert.ok(container);
  const root = createRoot(container);

  const session: GatewaySession = {
    token: "token_user_module",
    user: {
      app: "portal",
      uid: "user_1",
      tenantId: "tenant_portal",
      email: "user@portal.demo",
      displayName: "Portal User",
      roles: ["member"]
    }
  };

  const sessionStore = createBrowserSessionStore<GatewaySession>({
    tokenKey: "portal.user.token",
    sessionKey: "portal.user.session"
  });

  let meCalls = 0;
  const user: UserCommands = {
    login: async () => session,
    register: async () => session,
    me: async () => {
      meCalls += 1;
      return session;
    }
  };

  let snapshot: ReturnType<typeof useUserSession<GatewaySession>> | null = null;

  function Harness() {
    snapshot = useUserSession(user, sessionStore);
    return null;
  }

  try {
    await act(async () => {
      root.render(createElement(Harness));
    });

    assert.ok(snapshot);
    assert.equal(snapshot.session, null);
    assert.equal(snapshot.ready, true);
    assert.equal(snapshot.loading, false);

    await act(async () => {
      await snapshot?.login({
        app: "portal",
        email: "user@portal.demo",
        password: "demo123"
      });
    });

    assert.deepEqual(snapshot?.session, session);
    assert.deepEqual(sessionStore.read(), session);

    await act(async () => {
      await snapshot?.refresh();
    });

    assert.equal(meCalls, 1);
    assert.deepEqual(snapshot?.session, session);
  } finally {
    await act(async () => {
      root.unmount();
    });
    cleanup();
  }
});

test("useUserSession leaves loading false when no stored token exists", async () => {
  const { cleanup } = installDom();
  const container = document.getElementById("root");
  assert.ok(container);
  const root = createRoot(container);

  const sessionStore = createBrowserSessionStore<GatewaySession>({
    tokenKey: "empty.user.token",
    sessionKey: "empty.user.session"
  });

  let meCalls = 0;
  const user = createBrowserUserModule(
    {
      login: async () => {
        throw new Error("not used");
      },
      register: async () => {
        throw new Error("not used");
      },
      me: async () => {
        meCalls += 1;
        throw new Error("not used");
      }
    },
    sessionStore
  );

  let snapshot: ReturnType<typeof useUserSession<GatewaySession>> | null = null;

  function Harness() {
    snapshot = useUserSession(user);
    return null;
  }

  try {
    await act(async () => {
      root.render(createElement(Harness));
    });

    await flushEffects();

    assert.ok(snapshot);
    assert.equal(snapshot.session, null);
    assert.equal(snapshot.ready, true);
    assert.equal(snapshot.loading, false);
    assert.equal(meCalls, 0);
  } finally {
    await act(async () => {
      root.unmount();
    });
    cleanup();
  }
});
