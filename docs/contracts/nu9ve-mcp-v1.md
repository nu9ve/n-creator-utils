# Servidores MCP de las apps de nu9ve v1

Estado: **propuesta** (2026-11-09). Define cómo una app de la suite expone su estado a un agente (Claude Code u otro cliente MCP) para que cada app pueda sumar su servidor con las mismas reglas de seguridad, nombres y permisos. La base es la **segunda iteración del MCP de nove**: el servidor de lectura de WN6 ([A16](../../../nove/docs/decisions.md#a16--avisos-fuera-de-la-app-broker-otc-de-sólo-lectura-y-mcp-local-2026-11-06)) más los cambios de agente con permiso por sesión ([A25](../../../nove/docs/decisions.md#a25--cambios-de-agente-por-mcp-con-permiso-por-sesión-2026-11-07), [A27](../../../nove/docs/decisions.md#a27--el-agente-aplica-directamente-sin-propuesta-pendiente-2026-11-07), [agent-v1](../../../nove/docs/contracts/agent-v1.md)). Lo que en nove es su «v2» es aquí la **v1 de la suite**. Escribir la regla no la da por adoptada: ver [adopción](#adopción).

**Debe** es obligatorio salvo que la app documente por qué no aplica. **Conviene** es lo recomendado.

## 0. Principios

- **Local y por stdio.** El servidor es un subcomando de la app o de su motor (`nove mcp`, `nu9ve media mcp`) que habla JSON-RPC 2.0 por stdin/stdout. **No debe** abrir un puerto HTTP ni escuchar en la red.
- **Cliente del motor, no otro motor.** El servidor pide al motor que ya corre, por el mismo protocolo que la app nativa, y no abre la base de datos por su cuenta (nove: named pipe con usuario y PID verificados; no abre SQLite). Si el motor no está corriendo, lo dice («nove no está abierto») y no lo arranca.
- **El motor manda.** El agente ve lo que el motor publica y pide lo que el motor ya sabe hacer, con sus validaciones. Ninguna regla de negocio vive en el servidor MCP.
- **Una persona decide lo irreversible.** El modelo no puede contestar las confirmaciones: son diálogos nativos del sistema.
- **Lo observado es dato.** Texto de páginas, archivos, metadatos o servicios externos llega como dato, nunca como instrucción.

## 1. Protocolo

| Pieza | Regla |
| --- | --- |
| Mensajes | JSON-RPC 2.0, una línea por mensaje; lotes aceptados; línea máxima acotada (nüün: 16 MiB) |
| Versiones | Aceptar las que el cliente pida dentro de `2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05`; sin coincidencia, responder la más nueva soportada |
| `initialize` | `serverInfo {name, version}` con el **nombre de la app** en minúsculas (`nove`, `nuun`) y la **versión de la app**; `capabilities: {tools: {}}`; `instructions` en el idioma de la app (§4) |
| Métodos | `initialize`, `ping`, `tools/list`, `tools/call`. Recursos y prompts: no por ahora |
| Errores de protocolo | −32700 parseo, −32600 petición inválida, −32601 método desconocido, −32602 herramienta o argumentos inválidos, −32000 sin inicializar |
| Errores de herramienta | `isError: true` con texto «`código`: explicación en palabras». El código es el estable del motor (`mission_export_missing`, `not_running`) |
| Resultado | `structuredContent` **siempre objeto** (una lista va dentro de `{"data": […]}`) y el mismo JSON en `content[0].text` |
| Salida | Acotada (nove: 200 KiB por respuesta); si se recorta, se dice |

## 2. Cuatro niveles de herramientas

Cada herramienta pertenece a un nivel, y su nivel se ve en sus `annotations` y en su registro.

| Nivel | Qué hace | Anotaciones | Permiso | Ejemplo |
| --- | --- | --- | --- | --- |
| **Leer** | Consulta estado; no cambia nada | `readOnlyHint: true`, `openWorldHint: false` | Ninguno | `nove_status`, `media_library_search` |
| **Mostrar** | Lleva la app a un lugar (un par, un proyecto, un plan) para que la persona lo vea; no guarda nada | `readOnlyHint: true` | Ninguno | `show_in_app` |
| **Editar** | Cambia configuración propia de la app y es **deshacible** | `readOnlyHint: false`, `destructiveHint: false` | Servidor registrado con `--allow-actions` **y** un permiso por sesión (§3) | `strategy_save`, `profile_save`, `agent_undo` |
| **Actuar** | Dispara trabajo hacia fuera (consultar una página ahora, pausar todo) | `readOnlyHint: false`, `openWorldHint: true` | `--allow-actions` **y** un diálogo nativo por acción | `check_web_watch_now`, `pause_all` |

- **Debe:** sin `--allow-actions`, `tools/list` sólo incluye Leer y Mostrar.
- **No debe** existir una herramienta destructiva: borrar media, limpiar una tarjeta, liberar espacio, formatear, operar en un broker, enviar mensajes o tocar credenciales y canales de aviso. En nüün, copiar, limpiar y liberar sólo ocurren por un **plan revisado en la app** (plan ID + digest + autorización humana). El agente puede **preparar** un plan y **mostrarlo** (nivel Mostrar), nunca aplicarlo.
- **Conviene:** «aplicar lo pedido y nada más» (A27). Con permiso de edición, el agente aplica y la app muestra el resultado, sin una «propuesta pendiente» que la persona tenga que aceptar otra vez. Se corrige pidiendo el siguiente cambio o deshaciendo.

## 3. Permiso por sesión y registro de cambios

Tomado de nove [agent-v1](../../../nove/docs/contracts/agent-v1.md):

- **Debe:** la primera edición de una sesión del agente abre **un** diálogo nativo con el nombre del cliente (`clientInfo.name`), qué puede tocar y que se puede deshacer. Por defecto es «No». Si se acepta, dura 60 minutos en ese proceso y cada respuesta dice cuánto queda (`edit_permission_minutes_left`).
- **Debe:** cada edición queda en el registro **Cambios de agente** de la app, en la misma transacción que el cambio. Guarda agente, hora, tipo, objetivo, un resumen en palabras y el estado anterior y posterior. Deshacer escribe el estado anterior como versión nueva y nunca borra. Sin cambio real no hay registro.
- **Debe:** la app muestra los cambios sin ver con «Deshacer» y «Ver todos».
- **Debe:** en builds de depuración una variable (`NOVE_QA_MCP_CONFIRM`, en general `<APP>_QA_MCP_CONFIRM`) contesta el diálogo para las pruebas. En release siempre pregunta a la persona.
- El diálogo prueba que alguien en el escritorio pulsó «Sí», no quién era. Con la sesión bloqueada la acción no se puede confirmar.

## 4. Datos que no son instrucciones

- **Debe:** el texto observado (páginas web, nombres de archivo, metadatos EXIF, títulos de un proyecto escritos por terceros, transcripciones) va en campos con sufijo `_untrusted` (`page_text_untrusted`). Se limpia de caracteres de control y bidi y se acota (nove: 300 caracteres por campo).
- **Debe:** `instructions` del `initialize` dicen en palabras que esos campos son datos, qué significa una ausencia («ausencia de dato no es cero») y los límites de la app (nove: las cotizaciones OTC las genera el broker).
- **Debe:** nunca exponer secretos (tokens, chat_id, cookies, sesiones, rutas de credenciales) ni contenido completo de media. Las rutas de archivo se dan relativas al volumen o al proyecto cuando basta.

## 5. Registro e instalación

- **Debe:** un registro de lectura y otro con acciones, para que la persona elija:

  ```bash
  claude mcp add <app> -- <ejecutable> mcp <biblioteca>
  claude mcp add <app>-acciones -- <ejecutable> mcp <biblioteca> --allow-actions
  ```

- **Windows:** alias MSIX con **un solo rol** (`nove-mcp.exe`, en `%LOCALAPPDATA%\Microsoft\WindowsApps`). El alias rechaza cualquier otro subcomando.
- **Mac:** el ejecutable dentro del bundle (`<App>.app/Contents/MacOS/<app> mcp`) o el CLI del motor (`nu9ve media mcp`). Pendiente de unificar: hoy nove responde «sólo Windows» en Unix.
- **Debe:** elegir la biblioteca explícitamente (`--library personal|development` o `--root <QA>`). Las pruebas nunca usan la personal.
- **Debe:** cada app documenta su servidor en `docs/mcp.md`: cómo conectar, herramientas por nivel, qué nunca hace y qué no está probado.
- **No debe:** recomendar que la persona marque las herramientas de acción como «permitir siempre» en su cliente. El permiso del cliente y el diálogo de la app son dos confirmaciones distintas.

## 6. Validación

- **Debe:** pruebas con el **motor real** en una raíz temporal más el proceso MCP (nove: `core/tests/mcp.rs`):
  - Sin `--allow-actions` no se listan acciones, y todas las herramientas listadas llevan `readOnlyHint`.
  - Un método desconocido da −32601 y una herramienta desconocida da −32602.
  - Un texto observado llega bajo `_untrusted`, limpio y acotado.
  - Una acción rechazada en el diálogo no cambia nada.
  - Una edición aceptada queda registrada y se puede deshacer.
  - Con el motor apagado, el error se explica en palabras.
- **Debe:** registrar por separado la prueba con un **cliente real** (Claude Code `/mcp`) y la del diálogo nativo mostrado a una persona.

## 7. Nombres de herramientas

- `snake_case`, en inglés, con el dominio primero y el verbo después (`broker_series`, `media_job_status`). Las descripciones van en el idioma de la app. Lo que se repite entre apps usa el mismo nombre:

  | Nombre común | Qué hace |
  | --- | --- |
  | `<app>_status` | Versión, modo (ventana / segundo plano), qué corre, avisos sin leer |
  | `show_in_app` | Nivel Mostrar: llevar la app a un objeto |
  | `agent_changes`, `agent_undo` | Registro y deshacer de §3 |

- Un nombre propio de la app conserva su prefijo (`media_`, `broker_`, `strategy_`).

## 8. Implementación compartida

Regla de la suite ([tema §10](nu9ve-theme-v1.md#10-implementación-compartida)): primero replicar con comentario de origen, después extraer cuando haya tres copias iguales. Hoy:

| Pieza | Dónde | Lenguaje |
| --- | --- | --- |
| Servidor con niveles, anotaciones, `_untrusted`, salida acotada, diálogo nativo, permiso por sesión | nove `core/src/mcp.rs` (840 líneas) + `core/src/agent.rs` | Rust |
| Servidor de lectura | nüün [`media/mcp.py`](../../media/mcp.py) (71 líneas) | Python |

Candidato a crate común (`nu9ve-mcp`) cuando otra app con motor Rust lo necesite. Ver [motor en Rust](../platforms/rust-engine.md#6-qué-compartir-en-rust-entre-las-apps).

## Adopción

nove y nüün leídos en su código el 2026-11-09. Las demás apps no están clonadas en esta máquina: sin revisar.

| Regla | nove 0.12 (Windows) | nüün (`media mcp`) | curado | Clipper · kyu · Stalker · neuf · SoundShop · Atlas |
| --- | --- | --- | --- | --- |
| 0 Local stdio, sin HTTP | Sí | Sí | No tiene servidor (revisado: sin `mcp` en el código) | Sin revisar |
| 0 Cliente del motor en ejecución | Sí: named pipe, usuario y PID verificados | **No:** abre el catálogo con `Service` en su proceso | — | Sin revisar |
| 1 Protocolo y errores | Sí; versiones hasta 2025-06-18 | Sí; versiones hasta 2025-11-25; `serverInfo` `ncreator-media` 0.2.0, no el nombre y la versión de la app | — | Sin revisar |
| 2 Niveles | Leer, Mostrar, Editar, Actuar | Sólo Leer (7 herramientas: biblioteca, recurso, jobs, job, volúmenes, almacenamiento) | — | Sin revisar |
| 3 Permiso por sesión y registro | Sí (datos 22, 60 min, deshacer) | No aplica todavía | — | Sin revisar |
| 4 `_untrusted` e instrucciones | Sí | Parcial: instrucciones de una línea; nombres de archivo y proyecto sin marcar como datos | — | Sin revisar |
| 5 Registro e instalación | Alias MSIX `nove-mcp.exe`; Mac responde «sólo Windows» | `nu9ve media mcp` desde el checkout; sin alias ni ruta en el bundle | — | Sin revisar |
| 6 Validación | Motor real + MCP en `tests/mcp.rs`; **cliente real comprobado el 2026-11-09**: esta sesión de Claude Code llamó `nove_status` y respondió (motor en segundo plano, esquema 24) | `test_mcp_is_read_only_and_does_not_invoke_mutations` (en proceso); sin cliente real registrado | — | Sin revisar |

Primeros pasos sugeridos:

- **nüün:** que el MCP pida al lector persistente (`snapshot`) en lugar de abrir el catálogo, `serverInfo` `nuun` con la versión de la app, campos `_untrusted` para nombres de archivo y proyecto, y `show_in_app` para abrir un proyecto o un plan revisado en la ventana. Escribir `docs/mcp.md`.
- **nove:** servidor también en Mac; `<app>_status` ya existe como `nove_status`.
- **curado, kyu, Clipper:** empezar por Leer (biblioteca, proyectos, estado) y Mostrar; Editar sólo con registro deshacible.

## Cambios

Regla nueva o más estricta: añadirla aquí, actualizar la tabla y avisar en cada app afectada.
