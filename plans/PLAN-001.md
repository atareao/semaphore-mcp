# PLAN-001 — MCP en Rust para Semaphore UI

Issue: [#1](https://github.com/atareao/semaphore-mcp/issues/1)

## Fase 1 — Cliente REST tipado + tests wiremock

- [x] Estructura del crate, configuración y helpers de URLs.
- [x] `SemaphoreClient` con token passthrough (`Authorization: Bearer`).
- [x] Modelos tipados (`Project`, `Template`, `Task`, `Inventory`, `Repository`, `AccessKey`).
- [x] `list_projects` implementado con test wiremock.
- [ ] Implementar `list_templates`, `get_template`, `list_tasks`, `get_task`.
- [ ] Implementar `get_task_raw_output`, `stop_task`, `run_task`.
- [ ] Implementar `list_inventory`, `list_repositories`, `list_access_keys`.
- [ ] Tests wiremock para cada endpoint (incluido `run_task` con `template_id`).
- [ ] Cobertura de errores HTTP y de decodificación.

## Fase 2 — Servidor MCP y 12 tools

- [ ] Registrar `ping`.
- [ ] Registrar `list_projects`, `list_templates`, `get_template`, `run_task`.
- [ ] Registrar `get_task`, `list_tasks`, `stop_task`, `get_task_raw_output`.
- [ ] Registrar `list_inventory`, `list_repositories`, `list_access_keys`.
- [ ] Propagar el token del header MCP al cliente REST.
- [ ] Añadir `web_urls` a las respuestas de tareas.
- [ ] Tests de las tools (entrada, salida y errores).

## Fase 3 — Dockerfile distroless + E2E

- [ ] Dockerfile multi-stage con imagen distroless.
- [ ] Publicación de la imagen en el registro.
- [ ] E2E contra una instancia real de Semaphore: listar proyectos, ejecutar y
      consultar una tarea.
- [ ] Verificar `web_urls` con `SEMAPHORE_PUBLIC_URL`.
- [ ] Documentar el despliegue.

## Fuera de alcance v1

- Heurísticas de análisis de logs.
- CRUD de escritura (proyectos, plantillas, inventarios, repositorios, claves).
- Gestión de usuarios/roles.
- Transporte stdio.
