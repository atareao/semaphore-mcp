# AGENTS.md — semaphore-mcp

## Propósito

Servidor MCP en Rust para Semaphore UI. Sustituye al MCP Python
`cloin/semaphore-mcp`.

Alcance v1: 12 tools de lectura y ejecución (`ping`, `list_projects`,
`list_templates`, `get_template`, `run_task`, `get_task`, `list_tasks`,
`stop_task`, `get_task_raw_output`, `list_inventory`, `list_repositories`,
`list_access_keys`), token passthrough y URLs públicas correctas.

- `SEMAPHORE_URL`: URL interna para las llamadas REST.
- `SEMAPHORE_PUBLIC_URL`: URL pública opcional para construir `web_urls`; si no
  se define, se usa `SEMAPHORE_URL`.
- `SEMAPHORE_API_TOKEN`: opcional; si está definido, se usa siempre en lugar del
  token del cliente.
- Token passthrough: el `Authorization: Bearer` del cliente MCP se reenvía a
  Semaphore. El servidor no guarda un token fijo salvo `SEMAPHORE_API_TOKEN`.

Fuera de alcance v1: heurísticas de análisis de logs (el agente lee
`raw_output`), CRUD de escritura y gestión de usuarios/roles.

## Stack

- Rust estable, edición 2024 (`rust-version = "1.88"` por requisito de `rmcp`).
- `rmcp` con transporte HTTP streamable (sin stdio en v1).
- `reqwest` (rustls + json), `tokio`, `serde`/`serde_json`, `schemars`,
  `thiserror`, `tracing`/`tracing-subscriber`.
- Tests con `wiremock` + `tokio-test`.

## Estructura

- `src/lib.rs` — librería; `src/main.rs` — binario.
- `src/config.rs` — configuración y resolución de token.
- `src/client.rs` — cliente REST tipado y modelos.
- `src/tools.rs` — estado del servidor y registro de tools.
- `src/urls.rs` — construcción de `web_urls`.
- `tests/` — tests de integración. Unitarios en el mismo módulo (`#[cfg(test)]`).
- `docs/<feature>.md` — documentación por feature.
- `plans/PLAN-NNN.md` — planes del proyecto.

## Calidad obligatoria

Antes de cada commit:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Debe pasar todo en verde. Nunca commitear con warnings. El estado rojo solo se
admite como fase deliberada de TDD y debe quedar documentado en el plan.

## TDD

Tests primero: rojo → verde → refactor. Escribe el test que falla por la
funcionalidad que falta, implementa lo mínimo para pasarlo y refactoriza después.
Los tests de integración viven en `tests/`; los unitarios, en el mismo módulo.

## Git

Gitflow. `main` está protegida: los cambios entran por rama `feature/*` o
`fix/*` y PR. Nunca commits directos a `main`.

Commits con gitmoji + conventional commits:

- `✨ feat:`
- `🐛 fix:`
- `📝 docs:`
- `✅ test:`
- `♻️ refactor:`
- `🔧 chore:`
- `👷 ci:`

## Organización

- `plans/PLAN-NNN.md`: numeración secuencial. Al cerrar un plan, se renombra a
  `plans/PLAN-NNN-ARCHIVED.md`; no se borra.
- `docs/<feature>.md`: un fichero por feature.
- `HANDOFF.md` nunca se comitea; debe estar en `.gitignore`.

## Secretos

Ningún secreto en el repo. Los tokens llegan por variable de entorno o por la
cabecera del cliente. No loguear `Authorization` ni tokens.

## Diseño

- Errores tipados con `thiserror`.
- Sin `unwrap()`/`expect()` en rutas de producción.
- Timeouts explícitos en las peticiones HTTP.
- Sin heurísticas de análisis de logs: el agente lee `raw_output`.
- Token passthrough siempre que no haya `SEMAPHORE_API_TOKEN`.

## Licencia

Dual MIT/Apache-2.0 (`LICENSE-MIT` y `LICENSE-APACHE`).
