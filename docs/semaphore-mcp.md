# semaphore-mcp

## Qué es

Servidor MCP (Model Context Protocol) en Rust para Semaphore UI. Sustituye al
MCP Python `cloin/semaphore-mcp`. Expone operaciones de lectura y ejecución
sobre la API REST de Semaphore y devuelve enlaces públicos correctos.

## Ficheros implicados

| Fichero | Responsabilidad |
| --- | --- |
| `src/main.rs` | Arranque: configuración, logging, servidor HTTP y transporte MCP. |
| `src/config.rs` | `Config` desde variables de entorno y resolución del token. |
| `src/client.rs` | Cliente REST tipado (`SemaphoreClient`) y modelos de Semaphore. |
| `src/tools.rs` | Estado del servidor MCP y registro de tools. |
| `src/urls.rs` | Construcción de `web_urls` públicas. |

## Cómo funciona

1. `Config::from_env` lee `SEMAPHORE_URL`, `SEMAPHORE_PUBLIC_URL`, `MCP_HOST`,
   `MCP_PORT` y `SEMAPHORE_API_TOKEN`.
2. `SemaphoreServer` envuelve un `SemaphoreClient` apuntando a `SEMAPHORE_URL`.
3. El servidor se publica con `rmcp` + `axum` en `/mcp` usando el transporte
   HTTP streamable.
4. Cada tool recibe el token del header `Authorization: Bearer` de la petición
   MCP y lo reenvía a Semaphore. Si `SEMAPHORE_API_TOKEN` está definido, se usa
   siempre ese token.
5. Las respuestas incluyen `web_urls` construidas con `SEMAPHORE_PUBLIC_URL`
   (o `SEMAPHORE_URL` si no se define), sin barras duplicadas.

## Variables y configuración

Ver la tabla del [`README.md`](../README.md).

## Estado

Esqueleto inicial. El cliente tipado y los helpers de configuración/URLs están
implementados; las tools aún no están registradas. El test de `run_task` está en
rojo a propósito (TDD).

## Decisiones

- Transporte HTTP streamable únicamente (sin stdio) en v1.
- Token passthrough por petición; el cliente no guarda token fijo.
- Un solo crate con librería + binario para que los tests unitarios puedan
  importar los módulos.
- `rust-version = 1.88` porque `rmcp` 3.11 lo exige.

## Follow-ups

- Registrar las 12 tools.
- Tests E2E contra una instancia real de Semaphore.
- Dockerfile distroless y publicación de imagen.
