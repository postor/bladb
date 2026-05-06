# Flash Sale Design

This example is meant to show how Bladb can keep the frontend path short while moving complex cross-module coordination into events and workers.

## Primary modules

- `redis`
  - stock counters
  - wallet counters
  - hot-path rate limits
- `sql`
  - orders
  - payment records
  - user purchase history
- `mongo`
  - item snapshots
  - operator dashboards
  - replay-friendly event projections
- `nats + jetstream`
  - order-created events
  - stock-low events
  - durable retry lanes
  - payment timeout scheduling

## Synchronous path

The request path exposed to the frontend should stay short:

1. validate JWT and resolve `UID` / `TENANT_ID`
2. check purchase policy
3. reserve or decrement stock in `redis`
4. create pending order in `sql`
5. write outbox event
6. return to the caller

The frontend should not coordinate follow-up actions itself.

In the current example app, the browser reads a module-owned summary API first, then uses queue APIs for the purchase workflow:

```txt
GET /apps/flash-sale/summary
GET /users/me?app=flash-sale
POST /apps/flash-sale/queue
GET /apps/flash-sale/queue/:ticketId
```

`GET /apps/flash-sale/summary` now establishes or renews the anonymous browser identity through a cookie-backed session, and `GET /users/me?app=flash-sale` returns the same identity through the official `db.user` contract.

That keeps item, stock, wallet, and recent-order aggregation on the Rust side while preserving lower-level SQL and Redis policy fixtures underneath.

## Asynchronous path

After the synchronous path completes, Bladb should emit:

- `order.created`
- `stock.changed`

Those events can be handled by workers that:

- build analytics projections in `mongo`
- schedule payment timeout jobs in `jetstream`
- send notifications
- trigger recommendation or fraud pipelines

## Worker roles

Recommended workers for this example:

- `order.analytics-sync`
  - trigger: `order.created`
  - writes order snapshot to `mongo`
- `order.payment-timeout-scheduler`
  - trigger: `order.created`
  - publishes delayed job to `nats`
- `order.payment-timeout-handler`
  - trigger: delayed timeout subject or retry stream
  - cancels unpaid order
  - restores stock in `redis`
  - emits `order.cancelled`
- `stock.low-notifier`
  - trigger: `stock.changed`
  - alerts operators when thresholds are crossed

## Why this split matters

This keeps the high-concurrency purchase path cheap while still allowing the platform to coordinate multiple backends safely.

The official internal service path for this example is now:

- gateway -> module RPC through `natsService`
- domain events through `JetStream`
- worker scaling from queue depth and CPU on Kubernetes

There is also a gateway dry-run fixture for the user-scoped order read:

```txt
cargo run -p bladb-gateway -- apps/examples/flash-sale/policies/flash-sale.policy.yaml apps/examples/flash-sale/gateway/request.orders-read.json apps/examples/flash-sale/gateway/auth.buyer.json
```

The local example stack is served by the shared gateway binary with:

```txt
cargo run -p bladb-gateway -- serve 127.0.0.1:8787 apps/examples/gateway/local-gateway.yaml
```
