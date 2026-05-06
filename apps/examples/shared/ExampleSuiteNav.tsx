import "./example-suite.css";
import { getExampleSuite, type ExampleSuiteId } from "./exampleSuite";

export function ExampleSuiteNav({ currentApp }: { currentApp: ExampleSuiteId }) {
  const suite = getExampleSuite();
  const currentIndex = suite.findIndex((item) => item.id === currentApp);

  return (
    <nav aria-label="Example suite" className="example-suite-nav">
      <div className="example-suite-header">
        <span className="example-suite-eyebrow">Example suite</span>
        <strong>Jump across the official module demos</strong>
        <p>
          These examples are meant to be explored together: anonymous business flows first, then
          the dedicated `db.user` workbench when you want to inspect auth behavior directly.
        </p>
      </div>

      <div className="example-suite-progress">
        <span>Step {currentIndex + 1} of {suite.length}</span>
        <strong>{suite[currentIndex]?.stage ?? "Current example"}</strong>
      </div>

      <div className="example-suite-grid">
        {suite.map((item) => {
          const active = item.id === currentApp;

          return (
            <a
              aria-current={active ? "page" : undefined}
              className="example-suite-card"
              data-active={active ? "true" : "false"}
              href={item.url}
              key={item.id}
            >
              <div className="example-suite-meta">
                <span className="example-suite-mode">{item.stage}</span>
                <span className="example-suite-state">{active ? "Active now" : "Open demo"}</span>
              </div>
              <strong>{item.title}</strong>
              <p>{item.summary}</p>
              <div className="example-suite-stack">
                {item.modules.map((moduleName) => (
                  <code key={`${item.id}-${moduleName}`}>{moduleName}</code>
                ))}
              </div>
              <small className="example-suite-focus">{item.developerFocus}</small>
            </a>
          );
        })}
      </div>
    </nav>
  );
}
