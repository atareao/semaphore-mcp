# semaphore-mcp

MCP server for [Semaphore UI](https://semaphoreui.com/), written in Rust.

> **Status.** v1 feature-complete: the typed REST client covers the 11 Semaphore
> endpoints, the 12 MCP tools (those 11 plus `ping`) are registered over the
> streamable HTTP transport, and task results come back with public `web_urls`.
> See [`plans/PLAN-001.md`](plans/PLAN-001.md).

## Why

It replaces the Python `cloin/semaphore-mcp` server with a single Rust binary:
typed REST client, token passthrough, HTTP streamable transport and correct
public URLs for task results.

## Configuration

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `SEMAPHORE_URL` | yes | — | Internal Semaphore URL used for REST calls. |
| `SEMAPHORE_PUBLIC_URL` | no | `SEMAPHORE_URL` | Public URL used to build `web_urls`. |
| `SEMAPHORE_API_TOKEN` | no | — | Fixed token. When set, it always wins over the request header. |
| `MCP_HOST` | no | `0.0.0.0` | Address the MCP server binds to. |
| `MCP_PORT` | no | `8090` | Port the MCP server binds to. |

Token passthrough: the server forwards the client's `Authorization: Bearer`
header to Semaphore. It does not store a token unless `SEMAPHORE_API_TOKEN` is
configured.

## Build and run

```bash
cargo build --release
SEMAPHORE_URL=http://semaphore:3000 ./target/release/semaphore-mcp
```

The MCP endpoint is served at `http://<MCP_HOST>:<MCP_PORT>/mcp` using the
streamable HTTP transport. Stdio is not supported in v1.

Container deployment with Podman/Docker and Compose is documented in
[`docs/semaphore-mcp.md`](docs/semaphore-mcp.md#despliegue-con-contenedores).

## Tests

```bash
cargo test
```

The suite uses `wiremock` to fake Semaphore and covers the REST client and the
MCP tools: request shapes, token passthrough, `web_urls` and typed errors.

## License

Licensed under either of

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT license ([`LICENSE-MIT`](LICENSE-MIT))

at your option.
