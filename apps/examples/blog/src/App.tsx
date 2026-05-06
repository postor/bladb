import { useGatewaySession, useLiveValue, useMutation } from "@bladb/react";
import { useState } from "react";
import {
  BLOG_APP,
  blogUser,
  createPost,
  listMyPosts,
  listPublishedPosts,
  type BlogComposerInput,
  type BlogSession
} from "./bladb";
import { ExampleSuiteNav } from "../../shared/ExampleSuiteNav";

export default function App() {
  const session = useGatewaySession<BlogSession>(blogUser);
  const published = useLiveValue(() => listPublishedPosts(), 3000, [session.session?.token]);
  const mine = useLiveValue(
    () => (session.session ? listMyPosts() : Promise.resolve([])),
    3000,
    [session.session?.token]
  );
  const [mode, setMode] = useState<"login" | "register">("login");
  const [loginEmail, setLoginEmail] = useState("editor@blog.demo");
  const [loginPassword, setLoginPassword] = useState("demo123");
  const [registerName, setRegisterName] = useState("New Blogger");
  const [registerEmail, setRegisterEmail] = useState("new-blogger@blog.demo");
  const [registerPassword, setRegisterPassword] = useState("demo123");
  const [composer, setComposer] = useState<BlogComposerInput>({
    title: "Shipping a small tenant-aware blog",
    summary: "A fresh post created through db.user + db.mongo.",
    body: "This editor flow proves that the user module and Mongo policy path can work together."
  });
  const [error, setError] = useState<string | null>(null);
  const [publishMessage, setPublishMessage] = useState<string | null>(null);

  const login = useMutation(async () => {
    return await session.login({
      app: BLOG_APP,
      email: loginEmail,
      password: loginPassword
    });
  });

  const register = useMutation(async () => {
    return await session.register({
      app: BLOG_APP,
      email: registerEmail,
      password: registerPassword,
      displayName: registerName
    });
  });

  const publish = useMutation(async () => {
    if (!session.session) {
      throw new Error("Login first to create a post");
    }

    const post = await createPost(session.session, composer);
    setPublishMessage(`Published "${post.title}" to the public feed as /${post.slug}.`);
    await Promise.all([published.refresh(), mine.refresh()]);
    return post;
  });

  const wrap = async (runner: () => Promise<unknown>) => {
    setError(null);
    setPublishMessage(null);
    try {
      await runner();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Unknown error");
    }
  };

  return (
    <main className="page">
      <ExampleSuiteNav currentApp="blog" />

      <section className="hero">
        <div>
          <p className="eyebrow">Mongo + user example</p>
          <h1>Blog</h1>
          <p className="lede">
            Public readers can open the blog immediately. Editors sign in with `db.user`, then
            create and review their own tenant-scoped posts through `db.mongo`.
          </p>
          <div className="hero-notes">
            <article className="hero-note">
              <span>Public path</span>
              <strong>`GET /apps/blog/posts`</strong>
              <p>Anonymous visitors load published posts immediately through the app-owned route.</p>
            </article>
            <article className="hero-note">
              <span>Editor path</span>
              <strong>`db.user` + `db.mongo`</strong>
              <p>Editors mint a session with `db.user`, then publish and query personal posts through authenticated Mongo policies.</p>
            </article>
          </div>
        </div>
        <div className="session-card">
          <span className="label">Editor session</span>
          <strong>{session.session?.user.displayName ?? "Signed out"}</strong>
          <small>{session.session?.user.email ?? "Use the seeded editor or register a new one."}</small>
          <span className={session.session ? "status-chip status-live" : "status-chip"}>
            {session.session ? "Editor features unlocked" : "Public reading mode"}
          </span>
          {session.session ? (
            <button className="ghost" onClick={() => session.logout()} type="button">
              Logout
            </button>
          ) : null}
        </div>
      </section>

      <section className="grid info-grid">
        <article className="panel">
          <span className="label">How this demo works</span>
          <h2>One public path, one editor path</h2>
          <ul className="plain-list">
            <li>Published posts load through the app-owned anonymous route `GET /apps/blog/posts`.</li>
            <li>Login and register run through `db.user` so the browser holds a real app-scoped session.</li>
            <li>Publishing uses `db.mongo("posts").insertOne(...)` behind the editor policy path.</li>
          </ul>
        </article>

        <article className="panel panel-accent">
          <span className="label">What to verify</span>
          <h2>Expected browser behavior</h2>
          <ul className="plain-list">
            <li>Signed-out visitors should immediately see seeded published posts.</li>
            <li>Signing in should unlock the composer and personal feed without leaving the page.</li>
            <li>Publishing should add the new post to both `Published posts` and `My posts`.</li>
          </ul>
        </article>

        <article className="panel panel-code">
          <span className="label">SDK shape</span>
          <h2>How a developer should think about this page</h2>
          <pre className="code-block">{`const session = await db.user.login({ app: "blog", email, password })
await db.mongo("posts").insertOne({
  title,
  summary,
  body
})
const published = await fetch("/apps/blog/posts")`}</pre>
        </article>
      </section>

      <section className="grid">
        <article className="panel">
          <span className="label">Auth</span>
          <div className="toggle-row">
            <button
              className={mode === "login" ? "toggle selected" : "toggle"}
              onClick={() => setMode("login")}
              type="button"
            >
              Login
            </button>
            <button
              className={mode === "register" ? "toggle selected" : "toggle"}
              onClick={() => setMode("register")}
              type="button"
            >
              Register
            </button>
          </div>

          {mode === "login" ? (
            <>
              <p className="muted">Seed editor: `editor@blog.demo` / `demo123`.</p>
              <label className="field">
                <span>Email</span>
                <input onChange={(event) => setLoginEmail(event.target.value)} value={loginEmail} />
              </label>
              <label className="field">
                <span>Password</span>
                <input
                  onChange={(event) => setLoginPassword(event.target.value)}
                  type="password"
                  value={loginPassword}
                />
              </label>
              <button className="primary" disabled={login.loading} onClick={() => void wrap(login.run)}>
                {login.loading ? "Signing in..." : "Login"}
              </button>
            </>
          ) : (
            <>
              <label className="field">
                <span>Display name</span>
                <input onChange={(event) => setRegisterName(event.target.value)} value={registerName} />
              </label>
              <label className="field">
                <span>Email</span>
                <input onChange={(event) => setRegisterEmail(event.target.value)} value={registerEmail} />
              </label>
              <label className="field">
                <span>Password</span>
                <input
                  onChange={(event) => setRegisterPassword(event.target.value)}
                  type="password"
                  value={registerPassword}
                />
              </label>
              <button
                className="primary secondary"
                disabled={register.loading}
                onClick={() => void wrap(register.run)}
              >
                {register.loading ? "Creating..." : "Register"}
              </button>
            </>
          )}

          {error ? <p className="banner banner-error">{error}</p> : null}
        </article>

        <article className="panel panel-accent">
          <span className="label">Composer</span>
          <h2>Publish a post</h2>
          <p className="muted">
            {session.session
              ? `Signed in as ${session.session.user.displayName}. New posts publish straight into the shared public feed.`
              : "Public readers can browse immediately, but the composer opens only after editor login."}
          </p>
          <label className="field">
            <span>Title</span>
            <input
              onChange={(event) => setComposer((current) => ({ ...current, title: event.target.value }))}
              value={composer.title}
            />
          </label>
          <label className="field">
            <span>Summary</span>
            <textarea
              onChange={(event) => setComposer((current) => ({ ...current, summary: event.target.value }))}
              rows={3}
              value={composer.summary}
            />
          </label>
          <label className="field">
            <span>Body</span>
            <textarea
              onChange={(event) => setComposer((current) => ({ ...current, body: event.target.value }))}
              rows={6}
              value={composer.body}
            />
          </label>
          <button className="primary" disabled={publish.loading || !session.session} onClick={() => void wrap(publish.run)}>
            {publish.loading ? "Publishing..." : "Publish post"}
          </button>
          <p className="muted">`db.mongo("posts").insertOne(...)` is protected by the editor session.</p>
          {publishMessage ? <p className="banner banner-success">{publishMessage}</p> : null}
        </article>
      </section>

      <section className="double-grid">
        <article className="panel">
          <span className="label">Published posts</span>
          <div className="stack-list">
            {(published.data ?? []).map((post) => (
              <article className="post-card" key={post.id}>
                <strong>{post.title}</strong>
                <small>{post.authorName} · {post.createdAt}</small>
                <p>{post.summary}</p>
              </article>
            ))}
            {!published.loading && (published.data ?? []).length === 0 ? (
              <p className="muted">No published posts yet.</p>
            ) : null}
          </div>
        </article>

        <article className="panel">
          <span className="label">My posts</span>
          <div className="stack-list">
            {(mine.data ?? []).map((post) => (
              <article className="post-card" key={post.id}>
                <strong>{post.title}</strong>
                <small>{post.slug}</small>
                <p>{post.summary}</p>
              </article>
            ))}
            {!session.session ? <p className="muted">Login to inspect your own editor feed.</p> : null}
            {session.session && !mine.loading && (mine.data ?? []).length === 0 ? (
              <p className="muted">No personal posts yet.</p>
            ) : null}
          </div>
        </article>
      </section>
    </main>
  );
}
