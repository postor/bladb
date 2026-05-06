# Blog Design

This example shows the future base use case for third-party modules built on top of the official user surface.

## What it proves

- public readers can load content immediately
- editors can sign in through `db.user`
- authenticated editors can create tenant-scoped posts through `db.mongo`
- the same browser app can combine official module auth with native-looking Mongo calls

## Browser verification

1. Start the stack with `pnpm dev:examples`.
2. Open the printed `blog` URL.
3. Confirm the published post list renders before login.
4. Login with `editor@blog.demo` / `demo123`.
5. Publish a new post.
6. Confirm it appears in both `Published posts` and `My posts`.
