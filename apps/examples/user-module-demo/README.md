# User Module Demo

This example is a dedicated verification surface for the official browser-facing user module contract.

It intentionally avoids the flash-sale, IoT, and ROS2 UIs so the auth lifecycle can be checked on its own:

- `db.user.login(...)`
- `db.user.register(...)`
- `db.user.me()`
- `db.user.logout()`

## Seed account

- email: `member@user.demo`
- password: `demo123`

## Browser verification

1. Start the local example stack with `pnpm dev:examples:local`.
2. Open `http://127.0.0.1:4177`.
3. Login with `member@user.demo` / `demo123`.
4. Confirm the `db.user.me()` snapshot panel shows the current session.
5. Click `Refresh me` and confirm the session remains valid.
6. Register a new account and confirm it becomes the active session immediately.
7. Click `Logout` and confirm the page returns to a signed-out state.
