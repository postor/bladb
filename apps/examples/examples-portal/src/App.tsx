import { getExampleSuite } from "../../shared/exampleSuite";

const viteEnv = (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env;

const gatewayUrl = viteEnv?.VITE_BLADB_URL ?? "http://127.0.0.1:8787";
const ros2BackendUrl = viteEnv?.VITE_EXAMPLE_ROS2_BACKEND_URL ?? "http://127.0.0.1:8080";
const portalUrl = viteEnv?.VITE_EXAMPLE_PORTAL_URL ?? "http://127.0.0.1:4172";

const seedCredentials = [
  { label: "Flash-sale buyer", value: "buyer@flash-sale.demo / demo123" },
  { label: "Blog editor", value: "editor@blog.demo / demo123" },
  { label: "IoT operator", value: "operator@iot.demo / demo123" },
  { label: "ROS2 operator", value: "operator@ros2.demo / demo123" },
  { label: "User-module member", value: "member@user.demo / demo123" },
];

const tour = [
  {
    step: "1. Start here",
    title: "Examples Portal",
    detail: "Use this page to understand the suite layout, resolved local URLs, and the recommended demo order.",
  },
  {
    step: "2. Anonymous flows",
    title: "Flash Sale, IoT, ROS2",
    detail: "Open the direct-entry business demos first so you can learn the app-owned API shape without auth UI friction.",
  },
  {
    step: "3. Mixed auth path",
    title: "Blog",
    detail: "See how public reads and authenticated editor writes can live together in one browser experience.",
  },
  {
    step: "4. Auth contract",
    title: "User Module Demo",
    detail: "Finish in the dedicated db.user workbench when you want to validate login, register, me, and logout explicitly.",
  },
];

export default function App() {
  const suite = getExampleSuite();
  const anonymousExamples = suite.filter((item) => item.stage === "Anonymous flow");
  const mixedExample = suite.find((item) => item.id === "blog");
  const contractExample = suite.find((item) => item.id === "user-module-demo");

  return (
    <main className="portal-page">
      <section className="hero">
        <div className="hero-copy">
          <p className="eyebrow">Official example suite</p>
          <h1>One entry point for every Bladb demo</h1>
          <p className="lede">
            This portal turns the example stack into a guided suite. Start with anonymous business
            flows, move into mixed auth plus data behavior, then inspect the standalone `db.user`
            contract in its dedicated workbench.
          </p>
        </div>

        <article className="hero-card">
          <span className="label">Resolved stack</span>
          <dl className="fact-list">
            <div>
              <dt>Portal</dt>
              <dd>{portalUrl}</dd>
            </div>
            <div>
              <dt>Gateway</dt>
              <dd>{gatewayUrl}</dd>
            </div>
            <div>
              <dt>ROS2 backend</dt>
              <dd>{ros2BackendUrl}</dd>
            </div>
          </dl>
        </article>
      </section>

      <section className="learning-strip">
        <article className="learning-card">
          <span className="label">Start with</span>
          <strong>Anonymous business demos</strong>
          <p>Use direct-entry examples first to learn app-owned routes and worker/stream behavior without auth UI noise.</p>
        </article>
        <article className="learning-card">
          <span className="label">Then inspect</span>
          <strong>Mixed public + authenticated state</strong>
          <p>Open `blog` when you want to see anonymous reads and signed-in writes coexist in one browser surface.</p>
        </article>
        <article className="learning-card">
          <span className="label">Finish with</span>
          <strong>The standalone auth contract</strong>
          <p>Use `user-module-demo` as the final contract check for `db.user.login`, `register`, `me`, and `logout`.</p>
        </article>
      </section>

      <section className="section-grid">
        <article className="panel">
          <span className="label">Recommended tour</span>
          <div className="tour-list">
            {tour.map((item) => (
              <article className="tour-card" key={item.step}>
                <small>{item.step}</small>
                <strong>{item.title}</strong>
                <p>{item.detail}</p>
              </article>
            ))}
          </div>
        </article>

        <article className="panel panel-accent">
          <span className="label">Seed credentials</span>
          <div className="credential-list">
            {seedCredentials.map((credential) => (
              <article className="credential-card" key={credential.label}>
                <strong>{credential.label}</strong>
                <code>{credential.value}</code>
              </article>
            ))}
          </div>
        </article>
      </section>

      <section className="section-grid">
        <article className="panel">
          <span className="label">Choose your next stop</span>
          <div className="focus-stack">
            <article className="focus-card">
              <small>Anonymous examples</small>
              <strong>{anonymousExamples.map((item) => item.title).join(", ")}</strong>
              <p>Best when you want to understand app APIs, queue/stream behavior, and seeded runtime identities first.</p>
            </article>
            {mixedExample ? (
              <article className="focus-card">
                <small>{mixedExample.stage}</small>
                <strong>{mixedExample.title}</strong>
                <p>{mixedExample.developerFocus}</p>
              </article>
            ) : null}
            {contractExample ? (
              <article className="focus-card">
                <small>{contractExample.stage}</small>
                <strong>{contractExample.title}</strong>
                <p>{contractExample.developerFocus}</p>
              </article>
            ) : null}
          </div>
        </article>

        <article className="panel panel-accent">
          <span className="label">Module map</span>
          <div className="module-grid">
            {suite.filter((item) => item.id !== "examples-portal").map((item) => (
              <article className="module-card" key={item.id}>
                <strong>{item.title}</strong>
                <div className="module-chip-row">
                  {item.modules.map((moduleName) => (
                    <code key={`${item.id}-${moduleName}`}>{moduleName}</code>
                  ))}
                </div>
                <p>{item.developerFocus}</p>
              </article>
            ))}
          </div>
        </article>
      </section>

      <section className="panel">
        <span className="label">Example apps</span>
        <div className="app-grid">
          {suite.map((item) => (
            <a className="app-card" href={item.url} key={item.id}>
              <div className="app-meta">
                <span>{item.stage}</span>
                <span>Open demo</span>
              </div>
              <strong>{item.title}</strong>
              <p>{item.summary}</p>
              <div className="module-chip-row">
                {item.modules.map((moduleName) => (
                  <code key={`${item.id}-card-${moduleName}`}>{moduleName}</code>
                ))}
              </div>
              <small>{item.developerFocus}</small>
              <code>{item.url}</code>
            </a>
          ))}
        </div>
      </section>
    </main>
  );
}
