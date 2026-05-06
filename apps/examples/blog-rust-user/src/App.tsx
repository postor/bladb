import { useGatewaySession, useLiveValue, useMutation } from "@bladb/react";
import { useEffect, useState } from "react";
import {
  BLOG_APP,
  attemptHackUpdate,
  blogUser,
  createPost,
  deleteMyPost,
  listMyPosts,
  listPublishedPosts,
  readPublishedPost,
  updateMyPost,
  type BlogComposerInput,
  type BlogPost,
  type BlogSession
} from "./bladb";
import { ExampleSuiteNav } from "../../shared/ExampleSuiteNav";

const defaultComposer: BlogComposerInput = {
  title: "Publishing through the Rust user module",
  summary: "The editor session is now minted by the Rust user service.",
  body: "This variant keeps the blog UI but swaps the user backend from the Node launcher to the Rust launcher."
};

export default function App() {
  const session = useGatewaySession<BlogSession>(blogUser);
  const published = useLiveValue(() => listPublishedPosts(), 3000, [session.session?.token]);
  const mine = useLiveValue(
    () => (session.session ? listMyPosts() : Promise.resolve([])),
    3000,
    [session.session?.token]
  );
  const [mode, setMode] = useState<"login" | "register">("login");
  const [view, setView] = useState<"plaza" | "manage">("plaza");
  const [selectedSlug, setSelectedSlug] = useState<string | null>(null);
  const [selectedPost, setSelectedPost] = useState<BlogPost | null>(null);
  const [selectedLoading, setSelectedLoading] = useState(false);
  const [loginEmail, setLoginEmail] = useState("editor@blog.demo");
  const [loginPassword, setLoginPassword] = useState("demo123");
  const [registerName, setRegisterName] = useState("Rust Blogger");
  const [registerEmail, setRegisterEmail] = useState("rust-blogger@blog.demo");
  const [registerPassword, setRegisterPassword] = useState("demo123");
  const [composer, setComposer] = useState<BlogComposerInput>(defaultComposer);
  const [editingPostId, setEditingPostId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [publishMessage, setPublishMessage] = useState<string | null>(null);
  const [hackMessage, setHackMessage] = useState<string | null>(null);
  const pageError =
    error ?? published.error?.message ?? mine.error?.message ?? session.error?.message ?? null;

  useEffect(() => {
    if (!selectedSlug) {
      setSelectedPost(null);
      return;
    }

    let cancelled = false;
    setSelectedLoading(true);
    void readPublishedPost(selectedSlug)
      .then((post) => {
        if (!cancelled) {
          setSelectedPost(post);
        }
      })
      .catch((caught) => {
        if (!cancelled) {
          setError(caught instanceof Error ? caught.message : "Failed to load article");
        }
      })
      .finally(() => {
        if (!cancelled) {
          setSelectedLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [selectedSlug]);

  useEffect(() => {
    if (!selectedSlug && (published.data ?? []).length > 0) {
      setSelectedSlug(published.data![0].slug);
    }
  }, [published.data, selectedSlug]);

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

  const savePost = useMutation(async () => {
    if (!session.session) {
      throw new Error("Login first to manage articles");
    }

    const result = editingPostId
      ? await updateMyPost(editingPostId, composer)
      : await createPost(session.session, composer);
    setPublishMessage(
      editingPostId
        ? `Updated "${result.title}" and kept ownership under your editor identity.`
        : `Published "${result.title}" to the public feed as /${result.slug}.`
    );
    setEditingPostId(null);
    setComposer(defaultComposer);
    await Promise.all([published.refresh(), mine.refresh()]);
    setSelectedSlug(result.slug);
    return result;
  });

  const removePost = useMutation(async (post: BlogPost) => {
    const result = await deleteMyPost(post.id);
    setPublishMessage(`Deleted "${post.title}" from your article list.`);
    if (selectedSlug === post.slug) {
      setSelectedSlug(null);
      setSelectedPost(null);
    }
    await Promise.all([published.refresh(), mine.refresh()]);
    return result;
  });

  const hackAttempt = useMutation(async () => {
    const victim = (published.data ?? []).find(
      (post) => post.authorUid !== session.session?.user.uid
    );
    if (!victim) {
      throw new Error("No foreign article is available for the security check");
    }

    try {
      await attemptHackUpdate(victim.id);
      throw new Error("Security regression: foreign article update unexpectedly succeeded");
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : "Unauthorized write blocked";
      setHackMessage(`Blocked as expected: ${message}`);
      return message;
    }
  });

  const wrap = async (runner: () => Promise<unknown>) => {
    setError(null);
    setPublishMessage(null);
    setHackMessage(null);
    try {
      await runner();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Unknown error");
    }
  };

  const startEditing = (post: BlogPost) => {
    setEditingPostId(post.id);
    setComposer({
      title: post.title,
      summary: post.summary,
      body: post.body,
      published: post.published
    });
    setView("manage");
    setPublishMessage(null);
    setHackMessage(null);
  };

  return (
    <main className="page">
      <ExampleSuiteNav currentApp="blog" />

      <section className="hero">
        <div>
          <p className="eyebrow">Mongo + Rust user module example</p>
          <h1>Blog Rust User</h1>
          <p className="lede">
            Readers browse the homepage plaza immediately. Editors manage only their own articles,
            and any attempt to tamper with someone else&apos;s content is rejected.
          </p>
          <div className="hero-notes">
            <article className="hero-note">
              <span>Plaza</span>
              <strong>`GET /apps/blog/posts` + `GET /apps/blog/posts/:slug`</strong>
              <p>The homepage shows published articles from every author in the tenant, even before login.</p>
            </article>
            <article className="hero-note">
              <span>Ownership</span>
              <strong>`/apps/blog/me/posts` only</strong>
              <p>The editor can create, update, and delete only articles whose `authorUid` matches the active session.</p>
            </article>
          </div>
        </div>
        <div className="session-card">
          <span className="label">Editor session</span>
          <strong>{session.session?.user.displayName ?? "Signed out"}</strong>
          <small>{session.session?.user.email ?? "Use the seeded blog editor or register a new one."}</small>
          <span className={session.session ? "status-chip status-live" : "status-chip"}>
            {session.session ? "Own-article controls enabled" : "Public plaza only"}
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
          <span className="label">What changed</span>
          <h2>Two separate surfaces</h2>
          <ul className="plain-list">
            <li>The homepage plaza lists published articles from every author.</li>
            <li>The manage surface is scoped to `GET /apps/blog/me/posts` and never exposes write controls for foreign content.</li>
            <li>The Rust-backed `db.user` session still drives every editor action.</li>
          </ul>
        </article>

        <article className="panel panel-accent">
          <span className="label">Security check</span>
          <h2>What should fail</h2>
          <ul className="plain-list">
            <li>Trying to update someone else&apos;s post through a forged request should be denied.</li>
            <li>Deleting someone else&apos;s article should be denied too.</li>
            <li>The plaza remains readable even while all write paths stay session-bound.</li>
          </ul>
        </article>

        <article className="panel panel-code">
          <span className="label">SDK shape</span>
          <h2>Developer view</h2>
          <pre className="code-block">{`await db.app("blog").get("posts")
await db.app("blog").get("me/posts")
await db.app("blog").patch("me/posts/:id", patch)
// forged writes to another author must fail`}</pre>
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

          {pageError ? <p className="banner banner-error">{pageError}</p> : null}
          {publishMessage ? <p className="banner banner-success">{publishMessage}</p> : null}
          {hackMessage ? <p className="banner banner-success">{hackMessage}</p> : null}
        </article>

        <article className="panel panel-accent">
          <span className="label">Mode</span>
          <h2>Switch between plaza and management</h2>
          <div className="toggle-row">
            <button
              className={view === "plaza" ? "toggle selected" : "toggle"}
              onClick={() => setView("plaza")}
              type="button"
            >
              Plaza
            </button>
            <button
              className={view === "manage" ? "toggle selected" : "toggle"}
              disabled={!session.session}
              onClick={() => setView("manage")}
              type="button"
            >
              Manage mine
            </button>
          </div>
          <p className="muted">
            {view === "plaza"
              ? "The public homepage shows other authors' work too."
              : "Management controls are scoped to your own author identity."}
          </p>
          <button
            className="ghost"
            disabled={!session.session || hackAttempt.loading}
            onClick={() => void wrap(hackAttempt.run)}
            type="button"
          >
            {hackAttempt.loading ? "Testing..." : "Attempt hacker edit on foreign post"}
          </button>
        </article>
      </section>

      {view === "plaza" ? (
        <section className="double-grid">
          <article className="panel">
            <span className="label">Homepage plaza</span>
            <div className="stack-list">
              {published.loading ? <p className="muted">Loading published posts...</p> : null}
              {(published.data ?? []).map((post) => (
                <button
                  className={selectedSlug === post.slug ? "post-card post-card-active post-button" : "post-card post-button"}
                  key={post.id}
                  onClick={() => setSelectedSlug(post.slug)}
                  type="button"
                >
                  <strong>{post.title}</strong>
                  <small>{post.authorName} · {post.createdAt}</small>
                  <p>{post.summary}</p>
                </button>
              ))}
              {!published.loading && (published.data ?? []).length === 0 ? (
                <p className="muted">No published posts yet.</p>
              ) : null}
            </div>
          </article>

          <article className="panel">
            <span className="label">Selected article</span>
            {selectedLoading ? <p className="muted">Loading article...</p> : null}
            {!selectedLoading && selectedPost ? (
              <div className="article-detail">
                <h2>{selectedPost.title}</h2>
                <p className="muted">
                  {selectedPost.authorName} · {selectedPost.createdAt} · /{selectedPost.slug}
                </p>
                <p className="detail-summary">{selectedPost.summary}</p>
                <div className="article-body">{selectedPost.body}</div>
                <p className="muted">
                  {session.session?.user.uid === selectedPost.authorUid
                    ? "You own this article and can switch to Manage mine to edit it."
                    : "This article belongs to another author. You can read it here, but any edit attempt will be rejected."}
                </p>
              </div>
            ) : null}
            {!selectedLoading && !selectedPost ? <p className="muted">Pick an article from the plaza.</p> : null}
          </article>
        </section>
      ) : (
        <section className="double-grid">
          <article className="panel panel-accent">
            <span className="label">My editor console</span>
            <h2>{editingPostId ? "Edit your post" : "Create a post"}</h2>
            <p className="muted">
              {session.session
                ? `Every save stays bound to ${session.session.user.displayName} (${session.session.user.uid}).`
                : "Login to manage your articles."}
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
                rows={8}
                value={composer.body}
              />
            </label>
            <div className="button-row">
              <button className="primary" disabled={savePost.loading || !session.session} onClick={() => void wrap(savePost.run)}>
                {savePost.loading ? "Saving..." : editingPostId ? "Save changes" : "Publish post"}
              </button>
              <button
                className="ghost"
                disabled={!editingPostId}
                onClick={() => {
                  setEditingPostId(null);
                  setComposer(defaultComposer);
                }}
                type="button"
              >
                Reset
              </button>
            </div>
          </article>

          <article className="panel">
            <span className="label">My posts</span>
            <div className="stack-list">
              {mine.loading && session.session ? <p className="muted">Loading your posts...</p> : null}
              {(mine.data ?? []).map((post) => (
                <article className="post-card" key={post.id}>
                  <strong>{post.title}</strong>
                  <small>
                    /{post.slug} · {post.createdAt}
                  </small>
                  <p>{post.summary}</p>
                  <div className="button-row">
                    <button className="ghost" onClick={() => startEditing(post)} type="button">
                      Edit
                    </button>
                    <button
                      className="ghost danger"
                      disabled={removePost.loading}
                      onClick={() => void wrap(() => removePost.run(post))}
                      type="button"
                    >
                      Delete
                    </button>
                  </div>
                </article>
              ))}
              {!session.session ? <p className="muted">Login to inspect your own editor feed.</p> : null}
              {session.session && !mine.loading && (mine.data ?? []).length === 0 ? (
                <p className="muted">No personal posts yet.</p>
              ) : null}
            </div>
          </article>
        </section>
      )}
    </main>
  );
}
