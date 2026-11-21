import { DurableObject } from "cloudflare:workers";

const PORT = 8080;
const INACTIVITY_TIMEOUT_MS = 2 * 60 * 1000;

export class MtrContainer extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    if (ctx.container?.running) {
      void ctx.blockConcurrencyWhile(() =>
        ctx.container.setInactivityTimeout(INACTIVITY_TIMEOUT_MS),
      );
      this.observeExit();
    }
  }

  async fetch(request) {
    this.starting ??= this.startAndWaitForPort().finally(() => {
      this.starting = undefined;
    });
    await this.starting;

    const url = new URL(request.url);
    url.protocol = "http:";
    url.host = "container";
    const forwarded = new Request(url, request);
    forwarded.headers.delete("host");
    return this.ctx.container.getTcpPort(PORT).fetch(forwarded);
  }

  async startAndWaitForPort() {
    const container = this.ctx.container;
    if (!container) {
      throw new Error("MTR_CONTAINER is not configured");
    }

    if (!container.running) {
      container.start({
        image: container.images.base,
        instance: "lite",
        enableInternet: true,
        env: {
          MTR_HTTP_ADDR: `0.0.0.0:${PORT}`,
        },
      });
    }

    this.observeExit();
    await container.setInactivityTimeout(INACTIVITY_TIMEOUT_MS);

    const port = container.getTcpPort(PORT);
    for (let attempt = 0; attempt < 100; attempt++) {
      try {
        const response = await port.fetch("http://container/health", {
          signal: AbortSignal.timeout(1000),
        });
        await response.body?.cancel();
        if (response.ok) return;
      } catch {
        // The port is not listening yet; retry the readiness probe.
      }
      await scheduler.wait(200);
    }
    throw new Error("MTR container did not become ready on port 8080");
  }

  observeExit() {
    if (this.monitoring) return;
    this.monitoring = this.ctx.container
      .monitor()
      .then(() => console.log("MTR container exited"))
      .catch((error) => console.error("MTR container failed", error))
      .finally(() => {
        this.monitoring = undefined;
      });
    this.ctx.waitUntil(this.monitoring);
  }
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname === "/health") {
      return env.MTR_CONTAINER.getByName("mtr-hk").fetch(request);
    }
    if (url.pathname === "/mcp" || url.pathname.startsWith("/mcp/")) {
      return env.MTR_CONTAINER.getByName("mtr-hk").fetch(request);
    }
    if (url.pathname === "/") {
      return Response.json({
        service: "mtr-hk-mcp",
        transport: "streamable-http",
        endpoint: "/mcp",
        health: "/health",
      });
    }
    return new Response("Not found", { status: 404 });
  },
};
