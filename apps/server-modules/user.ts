const state = {
  users: new Map<string, StoredUser>(),
  sessions: new Map<string, StoredSession>(),
  nextUserId: 5000,
  nextSessionId: 1,
};

seed();

export async function register(input: RegisterInput) {
  const app = normalizeApp(input.app);
  const email = normalizeEmail(input.email);
  const displayName = input.displayName.trim();
  const password = input.password;

  if (!displayName || password.trim().length < 6) {
    throw new Error(
      "register requires non-empty email, displayName, and a password with at least 6 characters",
    );
  }

  const key = userKey(app, email);
  if (state.users.has(key)) {
    throw new Error("user already exists");
  }

  const seeded = firstUserForApp(app);
  const userId = `u_${state.nextUserId++}`;
  const user: StoredUser = {
    app,
    uid: userId,
    tenantId: seeded?.tenantId ?? "tenant_local",
    email,
    password,
    displayName,
    roles: seeded?.roles ?? ["member"],
    anonymous: false,
  };

  state.users.set(key, user);
  return issueSession(user, "authenticated");
}

export async function login(input: LoginInput) {
  const app = normalizeApp(input.app);
  const email = normalizeEmail(input.email);
  const user = state.users.get(userKey(app, email));
  if (!user || user.password !== input.password) {
    throw new Error("invalid email or password");
  }

  return issueSession(user, "authenticated");
}

export async function me() {
  const token = readToken("me");
  const session = state.sessions.get(token);
  if (!session) {
    throw new Error("session expired or token is invalid");
  }

  renewSession(session);
  return toPublicSession(session);
}

export async function logout() {
  const token = readToken("logout");
  const revoked = state.sessions.delete(token);
  if (!revoked) {
    throw new Error("session expired or token is invalid");
  }

  return { revoked: true };
}

interface RegisterInput {
  app: string;
  email: string;
  password: string;
  displayName: string;
}

interface LoginInput {
  app: string;
  email: string;
  password: string;
}

interface StoredUser {
  app: string;
  uid: string;
  tenantId: string;
  email: string;
  password: string;
  displayName: string;
  roles: string[];
  anonymous: boolean;
}

interface StoredSession {
  token: string;
  user: StoredUser;
  sessionKind: "authenticated" | "anonymous";
  issuedAt: number;
  lastSeenAt: number;
  expiresAt: number;
}

function seed() {
  const seededUsers: StoredUser[] = [
    {
      app: "user-module-demo",
      uid: "u_4001",
      tenantId: "tenant_local",
      email: "member@user.demo",
      password: "demo123",
      displayName: "Demo Member",
      roles: ["member"],
      anonymous: false,
    },
    {
      app: "blog",
      uid: "u_5001",
      tenantId: "tenant_blog",
      email: "editor@blog.demo",
      password: "demo123",
      displayName: "Blog Editor",
      roles: ["editor"],
      anonymous: false,
    },
  ];

  for (const user of seededUsers) {
    state.users.set(userKey(user.app, user.email), user);
  }
}

function issueSession(user: StoredUser, sessionKind: "authenticated" | "anonymous") {
  const now = nowSeconds();
  const session: StoredSession = {
    token: `session-${user.app}-${state.nextSessionId++}`,
    user,
    sessionKind,
    issuedAt: now,
    lastSeenAt: now,
    expiresAt: now + 60 * 60 * 24 * 30,
  };

  state.sessions.set(session.token, session);
  return toPublicSession(session);
}

function renewSession(session: StoredSession) {
  const now = nowSeconds();
  session.lastSeenAt = now;
  session.expiresAt = now + 60 * 60 * 24 * 30;
}

function toPublicSession(session: StoredSession) {
  return {
    token: session.token,
    sessionKind: session.sessionKind,
    anonymous: session.sessionKind === "anonymous",
    issuedAt: session.issuedAt,
    lastSeenAt: session.lastSeenAt,
    expiresAt: session.expiresAt,
    user: {
      app: session.user.app,
      uid: session.user.uid,
      tenantId: session.user.tenantId,
      email: session.user.email,
      displayName: session.user.displayName,
      roles: session.user.roles,
      anonymous: session.user.anonymous,
    },
  };
}

function readToken(action: "me" | "logout") {
  const value = (globalThis as Record<string, unknown>).__bladbLauncherPayload;
  if (!value || typeof value !== "object") {
    throw new Error(`missing launcher payload for ${action}`);
  }

  const db = (value as Record<string, unknown>).db;
  if (!db || typeof db !== "object") {
    throw new Error(`missing db payload for ${action}`);
  }

  const user = (db as Record<string, unknown>).user;
  if (!user || typeof user !== "object") {
    throw new Error(`missing db.user payload for ${action}`);
  }

  const actionPayload = (user as Record<string, unknown>)[action];
  if (!actionPayload || typeof actionPayload !== "object") {
    throw new Error(`missing db.user.${action} payload`);
  }

  const token = (actionPayload as Record<string, unknown>).token;
  if (typeof token !== "string" || !token.trim()) {
    throw new Error("missing bearer token");
  }

  return token.trim().replace(/^Bearer\s+/i, "");
}

function firstUserForApp(app: string) {
  return [...state.users.values()].find((user) => user.app === app);
}

function normalizeApp(app: string) {
  const normalized = app.trim().toLowerCase();
  if (!normalized) {
    throw new Error("app is required");
  }

  return normalized;
}

function normalizeEmail(email: string) {
  const normalized = email.trim().toLowerCase();
  if (!normalized) {
    throw new Error("email is required");
  }

  return normalized;
}

function userKey(app: string, email: string) {
  return `${app}:${email}`;
}

function nowSeconds() {
  return Math.floor(Date.now() / 1000);
}
