# PLAN-002 — Despliegue en co1 y validación de la cabecera Host

**Issue:** [#3](https://github.com/atareao/semaphore-mcp/issues/3) · **Estado:** en curso · **Rama:** `fix/allowed-hosts` (mergeada en `main`, tag `v0.1.1`)

> Este plan se escribió con el trabajo ya en marcha: el fallo se descubrió al
> desplegar y se documenta aquí junto al resto del despliegue, no antes.

## Objetivo

Dejar el servidor MCP de Semaphore **desplegado y funcionando** en el servidor
`co1`, sustituyendo al MCP Python de `cloin`, y corregir el fallo que lo hacía
inservible detrás de un proxy inverso.

## Contexto

- `co1` (Contabo, Ubuntu 22.04) ejecuta Docker + Traefik v3 + Dockge. Los stacks
  viven en `/home/lorenzo/docker/dockge/stacks/<stack>`.
- Semaphore UI v2.19.12 se sirve en `https://semaphore.territoriolinux.es`
  (entrypoint `tailnet` de Traefik, red Docker externa `proxy`, DNS interno
  `http://semaphore:3000`).
- El tailnet (Headscale + MagicDNS) resuelve los nombres a `100.64.0.1` mediante
  `dns.extra_records`; el certificado lo emite Let's Encrypt por reto HTTP-01.
- La fuente de verdad del despliegue es el repo `/data/infra`
  (`deploy/semaphore-mcp/compose.yaml`, `docs/semaphore-mcp.md`).

## Fallo corregido

`rmcp 3.11` limita por defecto los `Host` aceptados a `localhost`, `127.0.0.1` y
`::1` (protección anti DNS-rebinding). Con `StreamableHttpServerConfig::default()`
el servidor respondía `403 Forbidden: Host header is not allowed` a cualquier
petición que llegase a través de Traefik: la imagen `0.1.0` era inservible detrás
de un proxy inverso.

## Tareas

- [x] Abrir el issue #3 y crear la rama `fix/allowed-hosts`.
- [x] `AllowedHosts { Default | Any | List }` y variable `MCP_ALLOWED_HOSTS` en `Config`.
- [x] Extraer `build_app()` a la librería para poder probar la validación de `Host`.
- [x] Tests: 4 unitarios de parseo + 7 de integración, verificados por mutación.
- [x] Documentar `MCP_ALLOWED_HOSTS` (sin wildcards; `*` a secas es el único valor especial) en `README.md`, `docs/semaphore-mcp.md` y `compose.yml`.
- [x] Auditoría con `@rust-reviewer`: APTO.
- [x] Añadir `semaphore-mcp.territoriolinux.es` → `100.64.0.1` en `dns.extra_records` de Headscale y retirar `mcp.territoriolinux.es`.
- [x] Crear la sonda inofensiva `scripts/e2e-probe.sh` en `/data/infra` y la plantilla `bash` `e2e-probe` (id 2) en Semaphore.
- [x] Fusionar el PR #4 por rebase, publicar el tag `v0.1.1` y construir la imagen multi-arquitectura en GHCR.
- [ ] Desplegar `0.1.1` en `co1` con `MCP_ALLOWED_HOSTS=semaphore-mcp.territoriolinux.es`.
- [ ] Verificar `initialize` de MCP de punta a punta por la URL pública.
- [ ] Ejecutar el E2E contra el Semaphore real y verificar los `web_urls`.
- [ ] Sincronizar `/data/infra` (`deploy/semaphore-mcp/compose.yaml` y `docs/semaphore-mcp.md`) con el despliegue nuevo.
- [ ] Añadir el servidor `semaphore` a `~/.config/opencode/opencode.jsonc` con el token por `{file:...}`.
- [ ] Escribir `docs/despliegue.md`.
- [ ] Escribir `HANDOFF.md` y archivar este plan como `PLAN-002-ARCHIVED.md`.

## Criterios de aceptación

- `POST https://semaphore-mcp.territoriolinux.es/mcp` con `initialize` devuelve
  200 y una sesión MCP válida.
- El E2E (`cargo test --test e2e_semaphore -- --ignored`) pasa contra el
  Semaphore real y los `web_urls` se construyen con `SEMAPHORE_PUBLIC_URL`.
- Las puertas del repo (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`) siguen en verde.
- `co1` y `/data/infra` describen exactamente el mismo despliegue.

## Decisiones tomadas

- **Imagen en GHCR** (`ghcr.io/atareao/semaphore-mcp`), paquete público, publicada
  por el workflow `publish.yml` al empujar un tag `v*`.
- **Puerto 8090**, el valor por defecto del binario, en lugar del 8000 del MCP viejo.
- **Reemplazo** del MCP Python, no convivencia: el host público es el nuevo y el
  antiguo queda retirado.
- **Token en passthrough** desde el cliente MCP; el stack de `co1` no guarda
  ninguna credencial de Semaphore.
- **Sonda E2E dedicada** (`e2e-probe`) para no lanzar la plantilla real
  `Aplicar site.yml`, que aplica configuración sobre la infraestructura.
