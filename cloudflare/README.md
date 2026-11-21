# Cloudflare deployment

The primary Cloudflare deployment is a **pure V8-isolate Worker**. It builds the
Rust core to `wasm32-unknown-unknown`, uses the Workers `fetch` API for upstream
HTTP, and serves the same 15 MCP tools as the native server.

There is no Cloudflare Container, Docker, Tokio runtime, TCP socket or stdio in
this path. The Worker is stateless between requests apart from its in-isolate
caches.

## Layout

```text
worker/
  Cargo.toml             wasm cdylib crate
  src/lib.rs             Workers fetch backend + JSON-RPC MCP handler
  build.sh               builds with Rust 1.89 + worker-build
  wrangler.jsonc         standalone config (deploy from worker/)
wrangler.jsonc           root config (deploy from the repository root)
cloudflare/
  src/index.js           legacy Container front door
  wrangler.container.jsonc  legacy Container config
```

The shared service layer lives in `src/`. Native-only code is behind the default
`native` feature, so the Worker builds with
`--no-default-features` and does not pull in `reqwest`, Tokio, axum, clap or
rmcp.

## Prerequisites

- Rust 1.89 with the WASM target;
- `worker-build` 0.8;
- Node.js / `npx`;
- a Cloudflare account and `npx wrangler login`.

```bash
rustup target add wasm32-unknown-unknown --toolchain 1.89.0
cargo install worker-build@0.8.0
```

`worker/build.sh` prepends the Rust 1.89 toolchain when it is installed, because
a Homebrew `rustc` can shadow `rustup` and lacks the WASM target.

## Deploy

From the repository root:

```bash
npx wrangler secret put MTR_MCP_API_KEY   # optional
npx wrangler deploy
```

Or from `worker/`:

```bash
npx wrangler deploy
```

The endpoint is:

```text
https://<worker-name>.<account>.workers.dev/mcp
```

Connect an HTTP MCP client:

```bash
claude mcp add --transport http mtr-hk \
  https://<worker-name>.<account>.workers.dev/mcp
```

The Worker uses POST-only, stateless Streamable HTTP JSON responses. It does not
expose legacy SSE; use the native `mtr-hk-mcp-http` binary when an SSE transport
is required.

## Authentication

When `MTR_MCP_API_KEY` is configured, `/mcp` requires either:

```http
Authorization: Bearer <key>
```

or:

```http
X-API-Key: <key>
```

`/health` stays public. Without a key the MCP endpoint is unauthenticated, so put
Cloudflare Access or OAuth in front of a public deployment if you need per-user
identity.

## Local check

No Docker engine is required:

```bash
npx wrangler dev --port 8787
curl http://localhost:8787/health
```

Exercise authentication with:

```bash
npx wrangler dev --port 8787 --var MTR_MCP_API_KEY:test-key
```

Then POST JSON-RPC to `http://localhost:8787/mcp`:

```bash
curl -sS http://localhost:8787/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
```

## Validate without deploying

```bash
npx wrangler deploy --dry-run
```

This runs the WASM build and validates the Worker bundle without uploading it.

## Legacy Container fallback

The previous container-backed Worker is retained for accounts that need the
native HTTP server inside a Cloudflare Container. It requires Docker and
Containers enabled on the account:

```bash
cd cloudflare
npx wrangler secret put MTR_MCP_API_KEY -c wrangler.container.jsonc   # optional
npx wrangler deploy -c wrangler.container.jsonc
```

The container config builds `../Dockerfile` for `linux/amd64`, starts the Rust
HTTP server on port 8080, and forwards `/mcp` and `/health` through the Worker.
