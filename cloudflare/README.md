# Cloudflare deployment

This directory contains a Worker front door for the Rust HTTP MCP server.

Cloudflare runs the server in a **Cloudflare Container**, not as a pure
`wasm32-unknown-unknown` Worker. That is deliberate:

- the server needs TCP/TLS and a persistent Tokio runtime;
- the official Cloudflare MCP examples use the Workers JavaScript runtime;
- rewriting the 15 tools and MTR network layer for a V8 isolate would duplicate
  the Rust implementation and lose the shared cache.

The Worker is still the public Cloudflare endpoint. It starts the container,
probes `/health`, and forwards `/mcp` and `/health` to it.

## Deploy

Prerequisites:

- a Cloudflare account with Containers enabled;
- Node.js and `npx`;
- Docker (or a Docker-compatible CLI) running locally, because Wrangler builds
  the `linux/amd64` image from the repository root `Dockerfile`;
- `npx wrangler login`.

From the repository root:

```bash
npx wrangler deploy
```

Cloudflare builds the image, pushes it to the Cloudflare registry, and deploys
the Worker. The MCP endpoint is:

```text
https://<worker-name>.<account>.workers.dev/mcp
```

Connect an HTTP MCP client:

```bash
claude mcp add --transport http mtr-hk \
  https://<worker-name>.<account>.workers.dev/mcp
```

The endpoint is unauthenticated by default. Put Cloudflare Access, OAuth, or a
bearer-token check in front of it before exposing it publicly.

## Local Worker check

`npx wrangler dev` also needs a working Docker engine because it starts the
container locally:

```bash
npx wrangler dev
```

Then use `http://localhost:8787/mcp` as the MCP URL.

## Transport

Cloudflare's current remote MCP transport is **Streamable HTTP** at `/mcp`.
Legacy SSE is deprecated; it is not a separate endpoint here. The Rust server's
`/mcp` endpoint supports both JSON responses and SSE-framed responses.

## Pure WASM Worker status

A pure Rust Cloudflare Worker is not included. It requires replacing
`reqwest`/rustls, Tokio's multi-thread runtime, environment-based config, and the
stdio MCP transport with Workers-specific APIs. If a true V8-isolate Worker is a
hard requirement, the correct next step is a separate `mtr-hk-worker` crate with
a Worker `fetch` adapter and a rewritten HTTP backend, not this container.
