import { useEffect, useState } from "react";
import { type GatewaySessionState, useGatewaySession, useLiveValue, useMutation } from "@bladb/react";
import { flashSaleApi, flashSaleAuth, type FlashSaleSession, type QueueTicket } from "./bladb";

export default function App() {
  const auth = useGatewaySession<FlashSaleSession>(flashSaleAuth);

  if (!auth.ready && !auth.session) {
    return (
      <main className="page auth-page">
        <section className="hero">
          <p className="eyebrow">High concurrency demo</p>
          <h1>Flash Sale</h1>
          <p className="lede">Restoring buyer session...</p>
        </section>
      </main>
    );
  }

  if (!auth.session) {
    return <FlashSaleAuth auth={auth} />;
  }

  return (
    <FlashSaleDashboard
      onLogout={auth.logout}
      session={auth.session}
    />
  );
}

function FlashSaleAuth({
  auth
}: {
  auth: GatewaySessionState<FlashSaleSession>;
}) {
  const [loginEmail, setLoginEmail] = useState("buyer@flash-sale.demo");
  const [loginPassword, setLoginPassword] = useState("demo123");
  const [registerName, setRegisterName] = useState("New Buyer");
  const [registerEmail, setRegisterEmail] = useState("new-buyer@flash-sale.demo");
  const [registerPassword, setRegisterPassword] = useState("demo123");
  const [error, setError] = useState<string | null>(null);

  const login = useMutation(async () => {
    return await auth.login({
      app: "flash-sale",
      email: loginEmail,
      password: loginPassword
    });
  });

  const register = useMutation(async () => {
    return await auth.register({
      app: "flash-sale",
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
    <main className="page auth-page">
      <section className="hero">
        <p className="eyebrow">High concurrency demo</p>
        <h1>Flash Sale</h1>
        <p className="lede">
          Complete the buyer flow first: sign in, join the reservation queue, then poll for the
          final order result without leaving the page.
        </p>
      </section>

      <section className="auth-grid">
        <article className="panel panel-accent">
          <span className="label">Demo sign-in</span>
          <h2>Login</h2>
          <p className="muted">
            Seed account: <code>buyer@flash-sale.demo</code> / <code>demo123</code>
          </p>
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
        </article>

        <article className="panel">
          <span className="label">New buyer</span>
          <h2>Register</h2>
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
        </article>
      </section>

      {error ? <p className="banner banner-error">{error}</p> : null}
    </main>
  );
}

function FlashSaleDashboard({
  session,
  onLogout
}: {
  session: FlashSaleSession;
  onLogout: () => void;
}) {
  const [activeTicketId, setActiveTicketId] = useState<string | null>(null);
  const [settledTicketId, setSettledTicketId] = useState<string | null>(null);

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
      <section className="hero hero-row">
        <div>
          <p className="eyebrow">High concurrency demo</p>
          <h1>Flash Sale</h1>
          <p className="lede">
            Signed in as <strong>{session.user.displayName}</strong>. Purchases now go through a
            queue worker flow instead of directly mutating stock from the button click.
          </p>
        </div>
        <div className="session-card">
          <span className="label">Buyer session</span>
          <strong>{session.user.email}</strong>
          <small>{session.user.uid}</small>
          <button className="ghost" onClick={onLogout}>
            Logout
          </button>
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
          <span className="label">Session</span>
          <p className="metric">{session.user.roles.join(", ")}</p>
          <p className="muted">Tenant {session.user.tenantId}</p>
        </article>
      </section>

      <section className="panel action-row">
        <div>
          <span className="label">Purchase</span>
          <h2>Queue reservation flow</h2>
          <p className="muted">
            Login, join the queue, and poll the final status until the order is confirmed or sold
            out.
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
