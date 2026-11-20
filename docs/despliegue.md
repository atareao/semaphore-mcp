# Despliegue

## Qué es

Cómo se empaqueta, publica y despliega el servidor MCP de Semaphore. La imagen
es multi-arquitectura y pública en GHCR; en producción corre como contenedor
detrás de un proxy inverso (Traefik), sirviendo el transporte HTTP streamable en
la ruta `/mcp`.

## Ficheros implicados

| Fichero | Responsabilidad |
| --- | --- |
| `Dockerfile` | Imagen multi-stage: `rust:alpine3.23` (builder) → `alpine:3.23` (runtime). |
| `compose.yml` | Compose de ejemplo; lee las variables `SEMAPHORE_*` del entorno. |
| `.github/workflows/publish.yml` | Publica la imagen en GHCR al empujar un tag `v*`. |
| `src/config.rs` | `Config` desde el entorno y resolución del token. |
| `src/main.rs` | Arranque: bind, logging y `axum::serve` con `graceful_shutdown`. |
| `tests/e2e_semaphore.rs` | Test E2E `#[ignore]` contra un Semaphore real. |

## Cómo funciona

1. **Imagen.** `Dockerfile` multi-stage. El builder `rust:alpine3.23` instala
   `build-base`, `autoconf`, `musl-dev`, `pkgconfig`, `openssl`/`openssl-dev`/
   `openssl-libs-static`, `cmake`, `clang` y `perl`, compila con
   `cargo build --release` y aplica `strip`. El runtime `alpine:3.23` añade
   `ca-certificates` y `curl`, crea el usuario sin privilegios `mcp` (UID 1000),
   copia el binario, declara `EXPOSE 8090`, un `HEALTHCHECK` con
   `pgrep -x semaphore-mcp` y `CMD ["semaphore-mcp"]`.
2. **Publicación.** `.github/workflows/publish.yml` se dispara con un tag `v*`
   (o `workflow_dispatch`). Un job `image` construye `linux/amd64` y
   `linux/arm64` por separado con `push-by-digest=true` (arm64 bajo QEMU) y
   sube los digests como artefactos; un job `manifest` los descarga, ensambla el
   manifiesto con `docker buildx imagetools create` y aplica las etiquetas
   (`{{version}}`, `{{major}}.{{minor}}`, `latest` en tags `v*`, y `sha-`).
   El paquete `ghcr.io/atareao/semaphore-mcp` es **público**: ~10.7 MB
   comprimidos por arquitectura (3 capas).
3. **Por qué Alpine y no distroless.** Sigue la convención del resto de
   servidores MCP del monorepo. La única desviación es que el builder necesita
   `cmake`, `clang` y `perl` porque `aws-lc-rs` (vía `rustls`) compila C/asm en
   musl.
4. **CA bundle.** El binario usa `rustls` con certificados raíz **nativos**
   (`rustls-native-certs`), así que el runtime necesita
   `/etc/ssl/certs/ca-certificates.crt` (de ahí el `ca-certificates` de la
   imagen) para validar TLS contra Semaphore.
5. **Transporte.** HTTP streamable servido en `/mcp`
   (`src/lib.rs` hace `nest_service("/mcp", …)`; `src/main.rs` enlaza
   `<MCP_HOST>:<MCP_PORT>` y hace `axum::serve`).

## Variables y configuración

| Variable | Default | Obligatoria | Uso |
| --- | --- | --- | --- |
| `SEMAPHORE_URL` | — | sí | URL interna de la API REST. |
| `SEMAPHORE_PUBLIC_URL` | cae a `SEMAPHORE_URL` | no | URL pública para construir los `web_urls`. |
| `SEMAPHORE_API_TOKEN` | — | no | Token fijo; si se define gana sobre el del cliente. |
| `MCP_HOST` | `0.0.0.0` | no | Dirección de bind. |
| `MCP_PORT` | `8090` | no | Puerto de bind. |
| `MCP_ALLOWED_HOSTS` | solo loopback | no | Lista de `Host` aceptados (ver abajo). |
| `TZ` | — | no | Zona horaria del contenedor. |

**Token passthrough.** Sin `SEMAPHORE_API_TOKEN`, el servidor reenvía el token
del cliente MCP tomado del header `Authorization: Bearer <token>`
(`Config::resolve_token`). Así el stack de producción no guarda credenciales.

### `MCP_ALLOWED_HOSTS` — el detalle que hay que leer

`rmcp` valida la cabecera `Host` de cada petición (protección anti
DNS-rebinding). Sin `MCP_ALLOWED_HOSTS`, solo se aceptan `localhost`,
`127.0.0.1` y `::1`, de modo que **detrás de un proxy inverso responde
`403 Forbidden: Host header is not allowed` a todo**. Hay que declarar el nombre
público:

```bash
MCP_ALLOWED_HOSTS=semaphore-mcp.territoriolinux.es
```

- Lista separada por comas; espacios recortados, entradas vacías ignoradas.
- Una entrada sin puerto casa con cualquier puerto; con puerto exige ese puerto.
- Comparación exacta (host o `host:puerto`), insensible a mayúsculas.
  **No hay comodines**: `*.ejemplo.com` nunca casa.
- El único valor especial es `*` **a secas**, que desactiva la comprobación.
- Con una lista explícita, loopback **deja** de estar permitido.

## Cómo desplegar

### Local (build propio)

```bash
podman build -t semaphore-mcp:dev .   # o docker build
podman run -d --name semaphore-mcp -p 8090:8090 \
  -e SEMAPHORE_URL=https://semaphore.example.com \
  -e SEMAPHORE_PUBLIC_URL=https://semaphore.example.com \
  -e SEMAPHORE_API_TOKEN=... \
  semaphore-mcp:dev
```

O con el `compose.yml` incluido (`podman compose up -d`), definiendo las
variables `SEMAPHORE_*` en un `.env`.

### Producción (referencia real en `co1`)

Stack de Dockge en `co1`
(`/home/lorenzo/docker/dockge/stacks/semaphore-mcp/compose.yaml`), sobre la red
Docker externa `proxy`, detrás de Traefik con el entrypoint `tailnet` (solo
accesible por Tailscale) en `https://semaphore-mcp.territoriolinux.es/mcp` y
Semaphore en `http://semaphore:3000` por la red interna. La **fuente de verdad**
de ese despliegue vive en el repo `/data/infra`
(`deploy/semaphore-mcp/compose.yaml`):

```yaml
image: ghcr.io/atareao/semaphore-mcp:0.1.1
environment:
  SEMAPHORE_URL: http://semaphore:3000
  SEMAPHORE_PUBLIC_URL: https://semaphore.territoriolinux.es
  MCP_ALLOWED_HOSTS: semaphore-mcp.territoriolinux.es
  MCP_HOST: 0.0.0.0
  MCP_PORT: "8090"
  TZ: Europe/Madrid
```

### Cliente (opencode)

En `~/.config/opencode/opencode.jsonc`, como servidor remoto con
`"oauth": false` y el token inyectado por fichero (nunca escrito en el JSON):

```jsonc
"semaphore": {
  "type": "remote",
  "url": "https://semaphore-mcp.territoriolinux.es/mcp",
  "oauth": false,
  "headers": { "Authorization": "Bearer {file:~/.config/opencode/semaphore-mcp-token}" }
}
```

### Verificación

- `initialize` devuelve 200 con `mcp-session-id`.
- `tools/list` expone las 12 tools.
- A través del MCP desplegado funcionan `ping` (`pong`), `list_projects`,
  `list_templates` y `get_task` (con `web_urls` construidos con
  `SEMAPHORE_PUBLIC_URL`).

### E2E

Test `#[ignore]` en `tests/e2e_semaphore.rs`. Se ejecuta a mano con
`SEMAPHORE_URL`, `SEMAPHORE_PUBLIC_URL`, `SEMAPHORE_API_TOKEN`,
`SEMAPHORE_E2E_PROJECT_ID` y `SEMAPHORE_E2E_TEMPLATE_ID`. Necesita un proyecto
y una plantilla **inocuos** cuya tarea termine en `success` (la plantilla
`e2e-probe`, id 2, solo hace un `echo`):

```bash
SEMAPHORE_URL=… SEMAPHORE_PUBLIC_URL=… SEMAPHORE_API_TOKEN="$(cat ~/.config/opencode/semaphore-mcp-token)" \
SEMAPHORE_E2E_PROJECT_ID=1 SEMAPHORE_E2E_TEMPLATE_ID=2 \
cargo test --test e2e_semaphore -- --ignored --nocapture
```

## Decisiones

- **Imagen en GHCR** (`ghcr.io/atareao/semaphore-mcp`), paquete público,
  publicada por `publish.yml` al empujar un tag `v*`.
- **Multi-arquitectura** `linux/amd64` + `linux/arm64` por digest y manifiesto.
- **Alpine** (no distroless) por convención del monorepo; el builder añade
  `cmake`, `clang` y `perl` por `aws-lc-rs` en musl.
- **Usuario no root** `mcp` (UID 1000) y `HEALTHCHECK` por `pgrep`.
- **Puerto 8090** (default del binario), no el 8000 del MCP Python antiguo.
- **Reemplazo** del MCP Python `cloin`, no convivencia.
- **Token en passthrough**; el stack de `co1` no guarda credenciales.
- **Sonda E2E dedicada** (`e2e-probe`) para no lanzar `Aplicar site.yml`.

## Follow-ups

- El `AGENTS.md` de la raíz de `/data/infra` todavía describe el MCP Python
  antiguo.
- Los workflows emiten avisos de que `actions/checkout@v4`,
  `actions/upload-artifact@v4`, `actions/download-artifact@v4` y
  `docker/metadata-action@v5` apuntan a Node 20 mientras los runners fuerzan
  Node 24.
- La imagen se referencia por etiqueta, no por digest, y el tag `latest` se mueve
  en cada release.
