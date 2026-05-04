import { UID, key } from "@bladb/client";
import { useLiveValue, useMutation, useQuery } from "@bladb/react";
import { db } from "./bladb";

interface SaleItem {
  id: string;
  title: string;
  price: number;
  startsAt: string;
}

interface OrderRecord {
  id: string;
  status: string;
  quantity: number;
  createdAt: string;
}

const sku = "camera-pro";

export default function App() {
  const saleItem = useQuery(() =>
    db.mongo("flashsale_items").findOne<SaleItem>({ slug: sku })
  , []);

  const stock = useLiveValue(
    () => db.redis.get<number>(key`flashsale:${sku}:stock`),
    1500,
    []
  );

  const wallet = useLiveValue(
    () => db.redis.get<number>(key`${UID}_wallet`),
    3000,
    []
  );

  const orders = useQuery(
    () =>
      db.sql<OrderRecord[]>`
        select id, status, quantity, created_at as createdAt
        from orders
        where uid = ${UID} and sku = ${sku}
        order by created_at desc
        limit ${10}
      `,
    []
  );

  const purchase = useMutation(async () => {
    await db.redis.decrby(key`flashsale:${sku}:stock`, 1);
    await db.sql`
      insert into orders (uid, sku, quantity, status)
      values (${UID}, ${sku}, ${1}, ${"pending"})
    `;
    await Promise.all([stock.refresh(), orders.refresh()]);
  });

  return (
    <main className="page">
      <section className="hero">
        <p className="eyebrow">High concurrency demo</p>
        <h1>Flash Sale</h1>
        <p className="lede">
          The frontend keeps native SQL, Mongo, and Redis usage while backend policies still bind
          <code> UID </code>
          from JWT.
        </p>
      </section>

      <section className="grid">
        <article className="panel panel-accent">
          <span className="label">Featured item</span>
          <h2>{saleItem.data?.title ?? "Loading item..."}</h2>
          <p className="metric">${saleItem.data?.price ?? "--"}</p>
          <p className="muted">Starts at {saleItem.data?.startsAt ?? "--"}</p>
        </article>

        <article className="panel">
          <span className="label">Stock counter</span>
          <p className="metric">{stock.data ?? "--"}</p>
          <p className="muted">Backed by Redis hot-path counters.</p>
        </article>

        <article className="panel">
          <span className="label">My wallet</span>
          <p className="metric">{wallet.data ?? "--"}</p>
          <p className="muted">Reads use the reserved key template `UID_wallet`.</p>
        </article>
      </section>

      <section className="panel action-row">
        <div>
          <span className="label">Purchase</span>
          <h2>Single-click reservation flow</h2>
          <p className="muted">
            This skeleton intentionally shows direct Redis + SQL usage to preserve a native mental
            model.
          </p>
        </div>
        <button className="primary" disabled={purchase.loading} onClick={() => void purchase.run()}>
          {purchase.loading ? "Submitting..." : "Buy now"}
        </button>
      </section>

      <section className="panel">
        <span className="label">My recent orders</span>
        <div className="table">
          {(orders.data ?? []).map((order) => (
            <div className="row" key={order.id}>
              <strong>{order.id}</strong>
              <span>{order.status}</span>
              <span>x{order.quantity}</span>
              <time>{order.createdAt}</time>
            </div>
          ))}
          {!orders.loading && (orders.data ?? []).length === 0 ? (
            <p className="muted">No orders yet.</p>
          ) : null}
        </div>
      </section>
    </main>
  );
}
