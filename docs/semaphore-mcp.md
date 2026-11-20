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
   `MCP_PORT`, `MCP_ALLOWED_HOSTS` y `SEMAPHORE_API_TOKEN`.
2. `SemaphoreServer` envuelve un `SemaphoreClient` apuntando a `SEMAPHORE_URL`.
3. `build_app` publica el servidor con `rmcp` + `axum` en `/mcp` usando el
   transporte HTTP streamable y aplica la política de `Host` (`AllowedHosts`).
4. Cada tool recibe el token del header `Authorization: Bearer` de la petición
   MCP y lo reenvía a Semaphore. Si `SEMAPHORE_API_TOKEN` está definido, se usa
   siempre ese token.
5. Las respuestas incluyen `web_urls` construidas con `SEMAPHORE_PUBLIC_URL`
   (o `SEMAPHORE_URL` si no se define), sin barras duplicadas.

## Variables y configuración

Ver la tabla del [`README.md`](../README.md).

## Despliegue con contenedores

El repositorio incluye un `Dockerfile` (Alpine multi-stage, binario musl) y un
`compose.yml`. La imagen final corre como usuario no root `mcp` (UID 1000) y
expone el puerto `8090`.

### Build

```bash
podman build -t semaphore-mcp:dev .
# alternativa con Docker
docker build -t semaphore-mcp:dev .
```

### Run

```bash
podman run -d --name semaphore-mcp -p 8090:8090 \
  -e SEMAPHORE_URL=https://semaphore.example.com \
  -e SEMAPHORE_PUBLIC_URL=https://semaphore.example.com \
  -e SEMAPHORE_API_TOKEN=... \
  semaphore-mcp:dev
```

### Compose

```bash
podman compose up -d
```

Usa el `compose.yml` incluido. Las tres variables `SEMAPHORE_*`
(`SEMAPHORE_URL`, `SEMAPHORE_PUBLIC_URL`, `SEMAPHORE_API_TOKEN`) se leen del
entorno; defínelas en un fichero `.env` junto al `compose.yml`.

### Variables

| Variable | Default | Obligatoria |
| --- | --- | --- |
| `MCP_HOST` | `0.0.0.0` | no |
| `MCP_PORT` | `8090` | no |
| `SEMAPHORE_URL` | — | sí |
| `SEMAPHORE_PUBLIC_URL` | cae a `SEMAPHORE_URL` | no |
| `SEMAPHORE_API_TOKEN` | — | no; si se define, gana sobre el `Authorization: Bearer` del cliente MCP |
| `MCP_ALLOWED_HOSTS` | solo loopback (`localhost`, `127.0.0.1`, `::1`) | no; lista separada por comas de nombres `Host` aceptados. `*` desactiva la comprobación |

### Host header validation

`rmcp` valida la cabecera `Host` de cada petición al transporte HTTP streamable y
responde `403 Forbidden: Host header is not allowed` cuando no está en la lista.
Por defecto solo se acepta loopback, así que **detrás de Traefik** el `Host` es el
nombre público y hay que declararlo:

```bash
MCP_ALLOWED_HOSTS=semaphore-mcp.territoriolinux.es
```

- Lista separada por comas, con espacios recortados y entradas vacías ignoradas.
- Una entrada sin puerto casa con cualquier puerto; con puerto exige ese puerto.
- La comparación es **exacta** (host, o `host:puerto`) e insensible a mayúsculas:
  **no hay comodines ni patrones**. Una entrada como `*.example.com` no casa
  nunca y falla en silencio (la petición se rechaza con 403).
- El único valor especial es `*` **a secas** (un solo asterisco, sin host
  alrededor): desactiva la comprobación y acepta cualquier `Host`. No es un
  patrón. Úsalo con criterio: solo si el endpoint no es accesible desde clientes
  no confiables.
- Un valor ausente, vacío o solo espacios deja el comportamiento por defecto
  (solo loopback).
- `MCP_ALLOWED_HOSTS` no es un secreto: aparece en el `Debug` de `Config`. El
  `SEMAPHORE_API_TOKEN` sigue redactado.

### Endpoint MCP

Se sirve en `http://<MCP_HOST>:<MCP_PORT>/mcp` (por defecto
`http://0.0.0.0:8090/mcp`) con transporte HTTP streamable.

### Notas

- El contenedor corre como usuario no root `mcp` (UID 1000).
- `HANDOFF.md` nunca se comitea (está en `.gitignore`).
- La publicación de la imagen en un registro queda pendiente (decisión sin
  cerrar).

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
- Publicación de la imagen en un registro (pendiente).
