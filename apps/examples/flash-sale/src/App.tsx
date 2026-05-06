import { useEffect, useState } from "react";
import { useLiveValue, useMutation } from "@bladb/react";
import { flashSaleApi, flashSaleUser, type FlashSaleSession, type QueueTicket } from "./bladb";
import { ExampleSuiteNav } from "../../shared/ExampleSuiteNav";

export default function App() {
  return <FlashSaleDashboard />;
}

function FlashSaleDashboard() {
  const [activeTicketId, setActiveTicketId] = useState<string | null>(null);
  const [settledTicketId, setSettledTicketId] = useState<string | null>(null);
  const [session, setSession] = useState<FlashSaleSession | null>(() => flashSaleUser.read());

  const summary = useLiveValue(() => flashSaleApi.summary(), 2500, []);

  const queueStatus = useLiveValue(
    () => (activeTicketId ? flashSaleApi.queueTicket(activeTicketId) : Promise.resolve(null)),
    1500,
    [activeTicketId]
  );

  const queueHistory = useLiveValue(() => flashSaleApi.queueHistory(), 2500, []);

  useEffect(() => {
    const ticket = queueStatus.data;
    if (!ticket || !isTerminal(ticket.status) || ticket.ticketId === settledTicketId) {
      return;
    }

    setSettledTicketId(ticket.ticketId);
    void Promise.all([summary.refresh(), queueHistory.refresh()]);
  }, [queueHistory, queueStatus.data, settledTicketId, summary]);

  useEffect(() => {
    if (!summary.data) {
      return;
    }

    let active = true;
    void flashSaleUser
      .me()
      .then((nextSession) => {
        if (active) {
          setSession(nextSession);
        }
      })
      .catch(() => undefined);

    return () => {
      active = false;
    };
  }, [summary.data?.identity.uid]);

  const purchase = useMutation(async () => {
    const sku = summary.data?.item.sku;
    if (!sku) {
      throw new Error("Flash-sale item is still loading");
    }

    const ticket = await flashSaleApi.queuePurchase({ quantity: 1, sku });
    setActiveTicketId(ticket.ticketId);
    setSettledTicketId(null);
    await Promise.all([queueHistory.refresh(), summary.refresh()]);
    return ticket;
  });

  return (
    <main className="page">
      <ExampleSuiteNav currentApp="flash-sale" />

      <section className="hero hero-row">
        <div>
          <p className="eyebrow">High concurrency demo</p>
          <h1>Flash Sale</h1>
          <p className="lede">
            Anonymous example mode is enabled. The gateway mints a cookie-backed identity, renews
            its lease on each open, and lets `db.user.me()` restore the same browser identity
            before the flash-sale worker flow continues.
          </p>
          <div className="hero-notes">
            <article className="hero-note">
              <span>Read path</span>
              <strong>`GET /apps/flash-sale/summary`</strong>
              <p>One app-owned read returns identity, stock, wallet, recent orders, and the runtime collaboration path in a single response.</p>
            </article>
            <article className="hero-note">
              <span>Session path</span>
              <strong>`GET /users/me?app=flash-sale`</strong>
              <p>After the summary request sets the cookie, `db.user.me()` resolves the same anonymous identity without a login form.</p>
            </article>
          </div>
        </div>
        <div className="session-card">
          <span className="label">Browser identity</span>
          <strong>{session?.user.displayName ?? summary.data?.identity.displayName ?? "Resolving..."}</strong>
          <small>{session?.user.email ?? summary.data?.identity.email ?? "cookie-backed anonymous identity"}</small>
          <small>{summary.data?.identity.uid ?? "--"} / {summary.data?.identity.tenantId ?? "--"}</small>
        </div>
      </section>

      <section className="grid">
        <article className="panel panel-accent">
          <span className="label">Featured item</span>
          <h2>{summary.data?.item.title ?? "Loading item..."}</h2>
          <p className="metric">${summary.data?.item.price ?? "--"}</p>
          <p className="muted">Starts at {summary.data?.item.startsAt ?? "--"}</p>
        </article>

        <article className="panel">
          <span className="label">Stock counter</span>
          <p className="metric">{summary.data?.stock ?? "--"}</p>
          <p className="muted">Served by the flash-sale module summary API.</p>
        </article>

        <article className="panel">
          <span className="label">My wallet</span>
          <p className="metric">{summary.data?.wallet ?? "--"}</p>
          <p className="muted">Session-bound wallet data comes back from the same app summary.</p>
        </article>

        <article className="panel">
          <span className="label">Runtime identity</span>
          <p className="metric">{summary.data?.identity.anonymous ? "anonymous" : "member"}</p>
          <p className="muted">
            {summary.data?.identity.sessionKind ?? "--"} session for tenant {summary.data?.identity.tenantId ?? "--"}
          </p>
        </article>
      </section>

      <section className="grid">
        <article className="panel">
          <span className="label">What this page demonstrates</span>
          <h2>Anonymous identity plus queue worker flow</h2>
          <p className="muted">
            The browser enters directly, gets a cookie-backed identity, restores it through
            `db.user.me()`, then enqueues a purchase without ever showing a login screen.
          </p>
        </article>

        <article className="panel">
          <span className="label">Module path</span>
          <h2>db + redis + worker collaboration</h2>
          <p className="muted">
            `GET /apps/flash-sale/summary` shows the redis and db read path. `POST /apps/flash-sale/queue`
            hands work to the worker lane, which then coordinates redis reservation and db order insertion.
          </p>
        </article>

        <article className="panel panel-code">
          <span className="label">SDK shape</span>
          <h2>How a frontend should think about it</h2>
          <pre className="code-block">{`const summary = await flashSaleApi.summary()
const me = await flashSaleUser.me()
const ticket = await flashSaleApi.queuePurchase({ sku, quantity: 1 })
const status = await flashSaleApi.queueTicket(ticket.ticketId)`}</pre>
        </article>

        <article className="panel">
          <span className="label">Backend ownership</span>
          <h2>What the browser does not control</h2>
          <ul className="stack-list">
            <li>The anonymous identity is minted and renewed by the trusted gateway, not the page.</li>
            <li>Queue position and final order status are resolved by worker-side processing.</li>
            <li>Stock, wallet, and order summaries are aggregated server-side before they reach the UI.</li>
          </ul>
        </article>
      </section>

      <section className="double-grid">
        <article className="panel">
          <span className="label">Read collaboration</span>
          <div className="table">
            {(summary.data?.runtime.readPath ?? []).map((stage) => (
              <div className="row" key={`${stage.role}-${stage.action}`}>
                <strong>{stage.role}</strong>
                <span>{stage.action}</span>
                <span>{stage.cluster ?? "--"}</span>
                <span>summary</span>
              </div>
            ))}
          </div>
        </article>

        <article className="panel">
          <span className="label">Write collaboration</span>
          <div className="table">
            {(summary.data?.runtime.writePath ?? []).map((stage) => (
              <div className="row" key={`${stage.role}-${stage.action}`}>
                <strong>{stage.role}</strong>
                <span>{stage.action}</span>
                <span>{stage.cluster ?? "--"}</span>
                <span>queue</span>
              </div>
            ))}
          </div>
        </article>
      </section>

      <section className="panel action-row">
        <div>
          <span className="label">Purchase</span>
          <h2>Queue reservation flow</h2>
          <p className="muted">
            Join the queue immediately, then poll the final status until the order is confirmed or
            sold out.
          </p>
        </div>
        <button
          className="primary"
          disabled={purchase.loading || !summary.data?.item.sku}
          onClick={() => void purchase.run()}
        >
          {purchase.loading ? "Joining queue..." : "Join queue"}
        </button>
      </section>

      {queueStatus.data ? (
        <section className="panel">
          <span className="label">Current ticket</span>
          <div className="ticket-card">
            <div>
              <strong>{queueStatus.data.ticketId}</strong>
              <p className="muted">{queueStatus.data.message}</p>
            </div>
            <span className={`status status-${queueStatus.data.status}`}>{queueStatus.data.status}</span>
            <div className="ticket-meta">
              <span>Position: {queueStatus.data.queuePosition ?? "done"}</span>
              <span>Order: {queueStatus.data.orderId ?? "--"}</span>
            </div>
          </div>
          <div className="table">
            {queueStatus.data.steps.map((step) => (
              <div className="row" key={`${step.at}-${step.role}-${step.action}`}>
                <strong>{step.role}</strong>
                <span>{step.action}</span>
                <span>{step.detail}</span>
                <time>{step.at}</time>
              </div>
            ))}
          </div>
        </section>
      ) : null}

      <section className="double-grid">
        <article className="panel">
          <span className="label">Queue history</span>
          <div className="table">
            {(queueHistory.data ?? []).map((ticket) => (
              <button
                className="row row-button"
                key={ticket.ticketId}
                onClick={() => setActiveTicketId(ticket.ticketId)}
                type="button"
              >
                <strong>{ticket.ticketId}</strong>
                <span>{ticket.status}</span>
                <span>{ticket.queuePosition ?? "--"}</span>
                <span>{ticket.orderId ?? "pending"}</span>
              </button>
            ))}
            {!queueHistory.loading && (queueHistory.data ?? []).length === 0 ? (
              <p className="muted">No queued purchases yet.</p>
            ) : null}
          </div>
        </article>

        <article className="panel">
          <span className="label">My recent orders</span>
          <div className="table">
            {(summary.data?.orders ?? []).map((order) => (
              <div className="row" key={order.id}>
                <strong>{order.id}</strong>
                <span>{order.status}</span>
                <span>x{order.quantity}</span>
                <time>{order.createdAt}</time>
              </div>
            ))}
            {!summary.loading && (summary.data?.orders ?? []).length === 0 ? (
              <p className="muted">No orders yet.</p>
            ) : null}
          </div>
        </article>
      </section>
    </main>
  );
}

function isTerminal(status: QueueTicket["status"]) {
  return status === "completed" || status === "failed";
}
