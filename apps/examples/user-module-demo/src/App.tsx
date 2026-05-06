import { useMutation, useUserSession } from "@bladb/react";
import { useState } from "react";
import {
  USER_MODULE_DEMO_APP,
  describeSessionEnvelope,
  describeSessionFacts,
  describeVerificationChecklist,
  userDemoUser,
  type UserModuleDemoSession
} from "./bladb";
import { ExampleSuiteNav } from "../../shared/ExampleSuiteNav";

type AuthMode = "login" | "register";
type UserModuleSessionState = ReturnType<typeof useUserSession<UserModuleDemoSession>>;

export default function App() {
  const sessionState = useUserSession<UserModuleDemoSession>(userDemoUser);

  if (!sessionState.ready && !sessionState.session) {
    return (
      <main className="shell shell-loading">
        <section className="hero-card hero-card-loading">
          <p className="hero-tag">Official users module demo</p>
          <h1>Restoring your browser session</h1>
          <p className="hero-copy">
            The page is checking the persisted bearer token and asking the gateway for the current
            signed-in user.
          </p>
        </section>
      </main>
    );
  }

  return (
    <main className="shell">
      <ExampleSuiteNav currentApp="user-module-demo" />
      <Hero sessionState={sessionState} />
      <section className="workspace">
        <AuthSurface sessionState={sessionState} />
        <SessionSurface sessionState={sessionState} />
      </section>
    </main>
  );
}

function Hero({
  sessionState
}: {
  sessionState: UserModuleSessionState;
}) {
  const checklist = describeVerificationChecklist(sessionState.session);

  return (
    <section className="hero-card">
      <div className="hero-copy-block">
        <p className="hero-tag">Official users module demo</p>
        <h1>Developer-facing verification for `db.user`</h1>
        <p className="hero-copy">
          This page is a focused workbench for the standalone users module. It keeps the contract
          small on purpose: mint a session, re-hydrate it with `me`, replace it with `register`,
          and clear it with `logout`.
        </p>
        <div className="hero-callout">
          <strong>What this proves</strong>
          <p>
            `db.user` is usable as a first-class module surface, not hidden lab logic. The same
            browser flow validates session storage, gateway identity resolution, and app-scoped
            user behavior.
          </p>
        </div>

        <div className="sdk-surface">
          <article className="sdk-card">
            <p className="eyebrow">Frontend contract</p>
            <pre>{`await db.user.login({ app, email, password })
await db.user.me()
await db.user.logout()`}</pre>
          </article>
          <article className="sdk-card">
            <p className="eyebrow">When to use this page</p>
            <p>
              Open this workbench after the anonymous demos and `blog` when you want to verify the
              standalone session API by itself, without other business-module concerns mixed in.
            </p>
          </article>
        </div>
      </div>

      <div className="hero-rail">
        <article className="rail-card rail-card-emphasis">
          <p className="eyebrow">Seed account</p>
          <h2>Ready to validate immediately</h2>
          <dl className="seed-grid">
            <div>
              <dt>Email</dt>
              <dd>
                <code>member@user.demo</code>
              </dd>
            </div>
            <div>
              <dt>Password</dt>
              <dd>
                <code>demo123</code>
              </dd>
            </div>
          </dl>
        </article>

        <article className="rail-card">
          <p className="eyebrow">Verification path</p>
          <ol className="step-list">
            {checklist.map((step) => (
              <li className={`step-item step-${step.status}`} key={step.label}>
                <span className="step-state" aria-hidden="true" />
                <div>
                  <strong>{step.label}</strong>
                  <p>{step.detail}</p>
                </div>
              </li>
            ))}
          </ol>
        </article>
      </div>
    </section>
  );
}

function AuthSurface({
  sessionState
}: {
  sessionState: UserModuleSessionState;
}) {
  const [mode, setMode] = useState<AuthMode>("login");
  const [error, setError] = useState<string | null>(null);
  const [loginEmail, setLoginEmail] = useState("member@user.demo");
  const [loginPassword, setLoginPassword] = useState("demo123");
  const [registerName, setRegisterName] = useState("Demo Member");
  const [registerEmail, setRegisterEmail] = useState("new-member@user.demo");
  const [registerPassword, setRegisterPassword] = useState("demo123");

  const login = useMutation(async () => {
    return await sessionState.login({
      app: USER_MODULE_DEMO_APP,
      email: loginEmail,
      password: loginPassword
    });
  });

  const register = useMutation(async () => {
    return await sessionState.register({
      app: USER_MODULE_DEMO_APP,
      email: registerEmail,
      password: registerPassword,
      displayName: registerName
    });
  });

  const wrap = async (runner: () => Promise<unknown>) => {
    setError(null);
    try {
      await runner();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Unknown auth error");
    }
  };

  return (
    <section className="column stack-gap">
      <article className="panel panel-strong">
        <div className="panel-header">
          <div>
            <p className="eyebrow">User auth</p>
            <h2>Drive the contract</h2>
          </div>
          <div className="toggle-row" role="tablist" aria-label="Auth mode">
            <button
              aria-selected={mode === "login"}
              className={mode === "login" ? "toggle selected" : "toggle"}
              onClick={() => setMode("login")}
              type="button"
            >
              Login
            </button>
            <button
              aria-selected={mode === "register"}
              className={mode === "register" ? "toggle selected" : "toggle"}
              onClick={() => setMode("register")}
              type="button"
            >
              Register
            </button>
          </div>
        </div>

        <p className="muted panel-intro">
          {mode === "login"
            ? "Start with the seeded member to verify the first browser session."
            : "Register a clean account to confirm the module immediately swaps the active session."}
        </p>

        {mode === "login" ? (
          <form
            className="form-grid"
            onSubmit={(event) => {
              event.preventDefault();
              void wrap(login.run);
            }}
          >
            <label className="field">
              <span>Email</span>
              <input
                autoComplete="email"
                onChange={(event) => setLoginEmail(event.target.value)}
                value={loginEmail}
              />
            </label>
            <label className="field">
              <span>Password</span>
              <input
                autoComplete="current-password"
                onChange={(event) => setLoginPassword(event.target.value)}
                type="password"
                value={loginPassword}
              />
            </label>
            <button className="primary-button" disabled={login.loading || sessionState.loading} type="submit">
              {login.loading || sessionState.loading ? "Signing in..." : "Login"}
            </button>
          </form>
        ) : (
          <form
            className="form-grid"
            onSubmit={(event) => {
              event.preventDefault();
              void wrap(register.run);
            }}
          >
            <label className="field">
              <span>Display name</span>
              <input
                autoComplete="name"
                onChange={(event) => setRegisterName(event.target.value)}
                value={registerName}
              />
            </label>
            <label className="field">
              <span>Email</span>
              <input
                autoComplete="email"
                onChange={(event) => setRegisterEmail(event.target.value)}
                value={registerEmail}
              />
            </label>
            <label className="field">
              <span>Password</span>
              <input
                autoComplete="new-password"
                onChange={(event) => setRegisterPassword(event.target.value)}
                type="password"
                value={registerPassword}
              />
            </label>
            <button className="primary-button alt-button" disabled={register.loading || sessionState.loading} type="submit">
              {register.loading || sessionState.loading ? "Creating..." : "Register"}
            </button>
          </form>
        )}

        {error ? <p className="notice notice-error">{error}</p> : null}
        {sessionState.error ? <p className="notice notice-error">{sessionState.error.message}</p> : null}
      </article>

      <article className="panel panel-soft">
        <p className="eyebrow">Contract checkpoints</p>
        <h2>What this module should guarantee</h2>
        <ul className="plain-list">
          <li>`db.user.login(...)` persists a bearer token in browser storage.</li>
          <li>`db.user.register(...)` returns a usable active session immediately.</li>
          <li>`db.user.me()` resolves the same signed-in member without a separate app client.</li>
          <li>`db.user.logout()` clears the local snapshot and leaves the page in a clean signed-out state.</li>
        </ul>
      </article>

      <article className="panel panel-soft">
        <p className="eyebrow">How app developers use it</p>
        <h2>Suggested integration order</h2>
        <ol className="ordered-list">
          <li>Use `login` or `register` to mint the browser session for an app scope.</li>
          <li>Use `me` on startup or refresh to re-hydrate that session from stored credentials.</li>
          <li>Pass the resulting session into other module calls that require user context.</li>
          <li>Use `logout` to clear both the local token and the in-memory user snapshot.</li>
        </ol>
      </article>
    </section>
  );
}

function SessionSurface({
  sessionState
}: {
  sessionState: UserModuleSessionState;
}) {
  const refreshMe = useMutation(async () => {
    return await sessionState.refresh();
  });
  const facts = describeSessionFacts(sessionState.session);
  const envelope = describeSessionEnvelope(sessionState.session);

  return (
    <section className="column stack-gap">
      <article className="panel panel-strong">
        <div className="panel-header">
          <div>
            <p className="eyebrow">Current session</p>
            <h2>Live `db.user.me()` state</h2>
          </div>
          <div className="action-row">
            <button
              className="secondary-button"
              disabled={refreshMe.loading}
              onClick={() => void refreshMe.run()}
              type="button"
            >
              {refreshMe.loading ? "Refreshing..." : "Refresh me"}
            </button>
            <button
              className="ghost-button"
              onClick={() => sessionState.logout()}
              type="button"
            >
              Logout
            </button>
          </div>
        </div>

        <div className="fact-grid">
          {facts.map((fact) => (
            <article className="fact-card" key={fact.label}>
              <span>{fact.label}</span>
              <strong>{fact.value}</strong>
            </article>
          ))}
        </div>

        <div className="envelope-grid">
          {envelope.map((fact) => (
            <article className="envelope-card" key={fact.label}>
              <span>{fact.label}</span>
              <strong>{fact.value}</strong>
            </article>
          ))}
        </div>

        <pre className="json-card">
          {JSON.stringify(sessionState.session, null, 2)}
        </pre>
      </article>

      <article className="panel">
        <p className="eyebrow">Reading the snapshot</p>
        <h2>What to inspect as you test</h2>
        <ul className="plain-list">
          <li>The status cards should flip immediately between `Signed out` and `Signed in`.</li>
          <li>The envelope cards should expose app scope, UID, email, and a readable token summary.</li>
          <li>The JSON block should match the active browser session after login, register, and refresh.</li>
          <li>After logout, every card should collapse back to the signed-out placeholder state.</li>
        </ul>
      </article>
    </section>
  );
}
