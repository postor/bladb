# bladb-gateway

Minimal gateway crate for request validation, policy authorization, and reserved-value resolution.

Current responsibilities:

- load policy manifests
- validate gateway requests
- match requests to policies
- resolve `UID`, `TENANT_ID`, and template values into executable request bodies
- dry-run request preparation from the command line

CLI usage:

```txt
cargo run -p bladb-gateway -- <policy.yaml> <request.json> [auth.json]
```

Server usage:

```txt
cargo run -p bladb-gateway -- serve 127.0.0.1:8787
```

Routes:

- `GET /health`
- `POST /execute`
