# Rust Server Launcher Notes

This repo now includes a first Rust analogue to the Node server launcher:

- crate: `crates/bladb-server`
- purpose: host server-module handlers behind the same subject pattern used by `@bladb/server`
- current transport support:
  - in-memory
  - HTTP invoke transport

## Current scope

The Rust launcher is intentionally minimal in v1:

- it registers handlers by `module + method`
- it exposes subjects like `bladb.app.<app>.module.<module>.<method>`
- it returns the same `{ ok, data } / { ok, error }` response envelope pattern
- it ships with a tiny CLI binary entry so native modules can be hosted as a standalone process

## What it enables

This is the foundation for:

- Rust-authored replacement `user` modules
- Rust-authored project-local server modules
- future native launcher auto-start from project config
- future `dll/so/dylib` loading if we decide to add an in-process native provider later

## What is not done yet

- no filesystem module scanning for Rust crates yet
- no config-driven launcher auto-spawn yet
- no NATS transport in the Rust launcher yet
- no shared cross-language manifest/contract layer yet
- no direct gateway integration that swaps Node launcher for Rust launcher automatically

## Recommended next step

Implement a real Rust `user` module on top of `bladb-server`, then add:

1. a `launcher.command` config field
2. process supervision from dev startup
3. gateway integration tests against the Rust launcher path
