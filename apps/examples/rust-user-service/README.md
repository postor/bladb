# Rust User Service Example

This example is the smallest project-local Rust service built on top of `crates/bladb-server`.

It exists to prove that a project can implement a `user`-style backend module in Rust and expose it
through the same subject-oriented launcher shape as the Node server launcher.

The local example stack now uses this service as the launcher-backed official `db.user` runtime for
apps such as `blog`, `flash-sale`, `iot-realtime`, `ros2-bridge`, and `user-module-demo`.

## Run

From the workspace root:

```txt
cargo run -p rust-user-service
```

Default URL:

```txt
http://127.0.0.1:8790
```

## Invoke

Health:

```txt
POST /invoke/bladb.app.blog.module.user.health
```

Login:

```txt
POST /invoke/bladb.app.blog.module.user.login
```

Payload:

```json
{
  "app": "blog",
  "module": "user",
  "method": "login",
  "input": {
    "email": "editor@blog.demo",
    "password": "demo123"
  }
}
```
