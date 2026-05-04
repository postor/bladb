# Flash Sale Design

This example is meant to show how Bladb can keep frontend code close to native SQL and Redis usage while moving complex cross-module coordination into events and workers.

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
- `kafka` or `redis-streams`
  - order-created events
  - stock-low events
- `mq`
  - delayed payment timeout jobs
  - notification jobs

## Synchronous path

The request path exposed to the frontend should stay short:

1. validate JWT and resolve `UID` / `TENANT_ID`
2. check purchase policy
3. reserve or decrement stock in `redis`
4. create pending order in `sql`
5. write outbox event
6. return to the caller

The frontend should not coordinate follow-up actions itself.

## Asynchronous path

After the synchronous path completes, Bladb should emit:

- `order.created`
- `stock.changed`

Those events can be handled by workers that:

- build analytics projections in `mongo`
- schedule payment timeout jobs in `mq`
- send notifications
- trigger recommendation or fraud pipelines

## Worker roles

Recommended workers for this example:

- `order.analytics-sync`
  - trigger: `order.created`
  - writes order snapshot to `mongo`
- `order.payment-timeout-scheduler`
  - trigger: `order.created`
  - publishes delayed job to `mq`
- `order.payment-timeout-handler`
  - trigger: delayed timeout job
  - cancels unpaid order
  - restores stock in `redis`
  - emits `order.cancelled`
- `stock.low-notifier`
  - trigger: `stock.changed`
  - alerts operators when thresholds are crossed

## Why this split matters

This keeps the high-concurrency purchase path cheap while still allowing the platform to coordinate multiple backends safely.
