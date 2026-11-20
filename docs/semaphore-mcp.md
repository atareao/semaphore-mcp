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

Cliente REST tipado completo (11 endpoints: `list_projects`, `list_templates`,
`get_template`, `run_task`, `get_task`, `list_tasks`, `stop_task`,
`get_task_raw_output`, `list_inventory`, `list_repositories`, `list_access_keys`)
con tests wiremock. Las 12 tools MCP (esos 11 endpoints más `ping`) están
registradas con `#[tool_router]` y `#[tool]` de `rmcp`, con tests de entrada,
salida y errores (Fase 2 completa).

## Notas de API

- La salida cruda de una tarea se obtiene en
  `GET /api/project/{project_id}/tasks/{task_id}/raw_output` (Semaphore 2.14+,
  [PR #2880](https://github.com/semaphoreui/semaphore/pull/2880)); no es
  `/output`. El cliente devuelve el texto tal cual, sin quitar ANSI ni analizar.
- `stop_task` hace `POST .../tasks/{task_id}/stop` con **cuerpo JSON
  obligatorio** (`{"force": false}`); Semaphore decodifica `struct{ Force bool
  }` y un POST sin cuerpo provoca `io.EOF` → **400**. Responde 204 sin cuerpo.
- `run_task` (`POST .../tasks`) crea la tarea con **201**. El contrato es:
  `template_id` obligatorio; `environment` es un **string** (enviarlo como
  objeto devuelve 400 "cannot unmarshal object ... of type string"); `debug`,
  `dry_run` y `limit` (lista de strings) no son campos de nivel superior, van
  anidados en `params`.
- Los fallos de decodificación JSON se reportan como `SemaphoreError::Decode`
  con la URL y el cuerpo de respuesta (nunca cabeceras ni tokens).
- `get_task_raw_output` devuelve el log en `structuredContent.raw_output` y
  además como bloque de texto (`content`) con el texto tal cual, ANSI incluido,
  para clientes que no consumen `structuredContent`.
- Las respuestas de tarea (`Task`) llevan `web_urls` construidas en el cliente;
  `SemaphoreServer::new` cablea `SEMAPHORE_PUBLIC_URL` mediante
  `SemaphoreClient::with_public_url`.

## Tools MCP

| Tool | Entrada | Salida |
| --- | --- | --- |
| `ping` | — | `pong` |
| `list_projects` | — | lista de proyectos |
| `list_templates` | `project_id` | lista de templates |
| `get_template` | `project_id`, `template_id` | template |
| `run_task` | `project_id`, `template_id` | tarea creada con `web_urls` |
| `get_task` | `project_id`, `task_id` | tarea con `web_urls` |
| `list_tasks` | `project_id` | tareas con `web_urls` |
| `stop_task` | `project_id`, `task_id` | `{stopped: true}` |
| `get_task_raw_output` | `project_id`, `task_id` | `{raw_output}` |
| `list_inventory` | `project_id` | inventario |
| `list_repositories` | `project_id` | repositorios |
| `list_access_keys` | `project_id` | access keys |

Los IDs son enteros. El token se toma de `SEMAPHORE_API_TOKEN` si está definido;
si no, del header `Authorization: Bearer` de la petición MCP. Si falta, la tool
devuelve un error `invalid_params`. Los errores del cliente se devuelven como
`CallToolResult` con `is_error: true` y el mensaje tipado.

## Decisiones

- Transporte HTTP streamable únicamente (sin stdio) en v1.
- Token passthrough por petición; el cliente no guarda token fijo.
- Timeouts explícitos en el cliente HTTP (30 s petición, 10 s conexión).
- Un solo crate con librería + binario para que los tests unitarios puedan
  importar los módulos.
- `rust-version = 1.88` porque `rmcp` 3.11 lo exige.

## Follow-ups

- Tests E2E contra una instancia real de Semaphore.
- Dockerfile distroless y publicación de imagen.
