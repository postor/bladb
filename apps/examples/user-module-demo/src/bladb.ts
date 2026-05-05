import { createBrowserAppModule, type GatewaySession } from "@bladb/client";

const viteEnv = (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env;

export const BLADB_URL = viteEnv?.VITE_BLADB_URL ?? "http://localhost:8787";
export const USER_MODULE_DEMO_APP = "user-module-demo";
export const USER_MODULE_DEMO_TOKEN_KEY = "bladb.user-module-demo.token";
export const USER_MODULE_DEMO_SESSION_KEY = "bladb.user-module-demo.session";

export type UserModuleDemoSession = GatewaySession;

export interface UserModuleDemoModuleOptions {
  baseUrl?: string;
  fetcher?: typeof fetch;
}

export interface SessionFact {
  label: string;
  value: string;
}

export interface VerificationStep {
  label: string;
  detail: string;
  status: "idle" | "active" | "ready";
}

export function createUserModuleDemoModule(options: UserModuleDemoModuleOptions = {}) {
  return createBrowserAppModule({
    baseUrl: options.baseUrl ?? BLADB_URL,
    appName: USER_MODULE_DEMO_APP,
    tokenKey: USER_MODULE_DEMO_TOKEN_KEY,
    sessionKey: USER_MODULE_DEMO_SESSION_KEY,
    routes: {},
    fetcher: options.fetcher
  });
}

export const userModuleDemoModule = createUserModuleDemoModule();
export const userDemoDb = userModuleDemoModule.db;
export const userDemoUser = userModuleDemoModule.user;
export const userDemoAuth = userModuleDemoModule.auth;

export function describeSessionFacts(session: UserModuleDemoSession | null): SessionFact[] {
  if (!session) {
    return [
      { label: "Status", value: "Signed out" },
      { label: "Current user", value: "None" },
      { label: "Tenant", value: "No active tenant" },
      { label: "Roles", value: "No active roles" }
    ];
  }

  const roles = session.user.roles.length > 0 ? session.user.roles.join(", ") : "No active roles";

  return [
    { label: "Status", value: "Signed in" },
    { label: "Current user", value: session.user.displayName },
    { label: "Tenant", value: session.user.tenantId },
    { label: "Roles", value: roles }
  ];
}

export function describeSessionEnvelope(session: UserModuleDemoSession | null): SessionFact[] {
  if (!session) {
    return [
      { label: "App scope", value: "Awaiting login" },
      { label: "UID", value: "Not resolved yet" },
      { label: "Email", value: "No active session" },
      { label: "Token", value: "No bearer token" }
    ];
  }

  return [
    { label: "App scope", value: session.user.app },
    { label: "UID", value: session.user.uid },
    { label: "Email", value: session.user.email },
    { label: "Token", value: truncateToken(session.token) }
  ];
}

export function describeVerificationChecklist(
  session: UserModuleDemoSession | null
): VerificationStep[] {
  if (!session) {
    return [
      {
        label: "Login with seeded account",
        detail: "Use member@user.demo / demo123 to mint the first bearer token.",
        status: "active"
      },
      {
        label: "Refresh the session",
        detail: "Run db.user.me() after login and confirm the same user comes back.",
        status: "idle"
      },
      {
        label: "Register a fresh member",
        detail: "Create a new account and confirm it immediately becomes the active session.",
        status: "idle"
      },
      {
        label: "Logout cleanly",
        detail: "Revoke the browser session and confirm the snapshot returns to signed out.",
        status: "idle"
      }
    ];
  }

  return [
    {
      label: "Login with seeded account",
      detail: "Use member@user.demo / demo123 to mint the first bearer token.",
      status: "ready"
    },
    {
      label: "Refresh the session",
      detail: "Run db.user.me() after login and confirm the same user comes back.",
      status: "active"
    },
    {
      label: "Register a fresh member",
      detail: "Create a new account and confirm it immediately becomes the active session.",
      status: "active"
    },
    {
      label: "Logout cleanly",
      detail: "Revoke the browser session and confirm the snapshot returns to signed out.",
      status: "active"
    }
  ];
}

function truncateToken(token: string): string {
  if (token.length <= 14) {
    return token;
  }

  return `${token.slice(0, 8)}...${token.slice(-5)}`;
}
