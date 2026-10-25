# Contratos del ecosistema de media

Actualización 2026-10-25: hay productor de handoff, recibos, servicio JSON y MCP read-only en este repo. El contrato entre apps sigue **en revisión, sin adopción del consumidor Clipper**. Estado completo: [STATUS](../STATUS.md). Propuesta inicial: 2026-10-24. Este directorio es el índice central de almacenamiento e intercambio entre n-creator-utils, Stalker y Clipper; Atlas participa como consumidor adicional. No declara adopción de cambios en los otros repos.

## Autoridad y compatibilidad

| Contrato | Autoridad | Estado |
| --- | --- | --- |
| Captura, archivos, reloj y audio `stalker-clipper/1`, revisión 1.1 | [Stalker](https://github.com/nu9ve/stalker/blob/46161c7/docs/clipper-contract.md), considerar notas locales actuales | Implementado por productor/consumidor con límites documentados en sus repos. No duplicar ese schema aquí. |
| Proyecto/timeline/perfiles y App Services | [Clipper](https://github.com/nu9ve/clipper/blob/e6981b1/apps/APP_CONTRACT.md) y [servicios](https://github.com/nu9ve/clipper/blob/e6981b1/apps/APP_SERVICES_CONTRACT.md) | Contratos existentes; no escribir proyectos desde el archivador. |
| Descubrimiento `nu9ve-inspection/1` | [inventory.py](../../manager/inventory.py) | Implementado aquí; observaciones sin garantía de importabilidad/integridad ni destinos. |
| Handoff de biblioteca `creator-media-handoff/1` | [Especificación](media-library-v1.md) + [schema](media-handoff.schema.json) | Productor `media/library.py` implementado; sin consumidor implementado. [Ejemplo](examples/handoff.json) usa datos ficticios. |
| Naming y carpetas de media v1 | [Especificación](media-naming-v1.md), productor [layout.py](../../media/layout.py) | Implementado aquí (planes esquema 2, `relayout`) y usado en el primer respaldo real. Stalker/Clipper sin adoptar. |
| Transferencia, resolución, relink, dependencias y publicación | [Especificación común](media-library-v1.md) | Transporte local/operaciones básicas implementados; relink ACK/leases y automatización de publicación pendientes. |

El índice central enlaza formatos propietarios; sólo se centralizan decisiones que cruzan repos. Si un contrato propio cambia, actualizar este índice y las pruebas de compatibilidad. Cambios incompatibles requieren nueva versión mayor; adiciones opcionales se preservan/ignoran sin reinterpretar datos desconocidos. No aceptar un major desconocido ni ejecutar metadata.

## Puertas de adopción

1. Revisión de Stalker: estabilidad de `completed`, revisiones del manifest, metadata de proyecto opcional y señal de finalización.
2. Revisión de Clipper: importación de handoff, selección de biblioteca, relink integral `capture_sessions`/items/voz/eventos/colas, orden y perfil como sugerencia.
3. n-creator-utils: planner y transferencias verificadas primero; después publicar handoff a partir de recibos reales. Un JSON que pasa schema no demuestra que existan bytes correctos.
4. Fixture compartida copiada del contrato con versiones fijadas: importa por ruta, importa por biblioteca, relocaliza, abre cola y exporta sin duplicar audio. ACK con revisión y hashes.
5. Sólo entonces habilitar retiro de ubicaciones viejas, previa revisión de consumidores no integrados como Final Cut.

## Estado de adopción por app

Cada sesión de Stalker o Clipper actualiza **su fila** al terminar, con fecha y commit de su repo.

| App | Contrato / sección | Estado | Fecha · commit | Notas y límites |
| --- | --- | --- | --- | --- |
| n-creator-utils | Productor: naming v1, handoff v1, interfaz local | Implementado | 2026-10-25 | Ver [STATUS](../STATUS.md). |
| Stalker | naming v1 «Adopción: Stalker», estado de archivo | Pendiente | — | Hoy nombra `Stalker_fecha` y graba en `~/Downloads`. |
| Clipper | naming v1 «Adopción: Clipper», interfaz local, handoff v1 | Pendiente | — | `expand_inputs` ya respeta el orden de la carpeta (comprobado en sólo lectura). |
| Atlas | library v1 como consumidor | Sin iniciar | — | — |

## Interfaz local estable para las apps (sólo lectura)

```text
python3 /Users/nu9ve/code/nu9ve/n-creator-utils/nu9ve.py media <comando>   # o `nu9ve media` si está instalado
```

- **Formato de respuesta:** stdout JSON `{"ok": true, "command": …, "data": …}` o `{"ok": false, "error": {"code", "message"}}`; código de salida 0/1.
- **Catálogo:** por defecto el del usuario. `--state-dir DIR` o `NU9VE_MEDIA_HOME` dan un catálogo aislado; **las pruebas de otras apps usan siempre uno aislado con fixtures**.

| Comando | Para qué | Datos |
| --- | --- | --- |
| `projects` | Selector de proyectos | `[{project, project_id, profile, assets, updated_at}]`, más reciente primero. |
| `sources --project P [--purpose edit\|archive]` | Quick import de un proyecto | `sources[]` en orden de captura: `order, asset_id, revision, kind (file\|camera_bundle\|stalker_session), session_id, path` (absoluto: archivo o carpeta `.stalker`), `name, volume, role, online, bytes, sha256` (de archivos), `captured_at` (hora local de inicio, sin zona) y `duration_seconds` (leídos del nombre fechado; `null` si no lo es), `alternatives[]` y `missing[]`. Aparte vienen `offline[]`, `removed_from_work[]` (borradas a mano de trabajo; apuntan al archivo) y `warnings[]` (`archive_copy_missing`). Prefiere la copia de trabajo en línea y comprueba presencia y tamaño, sin hashear. |
| `captures ROOT` | Estado de archivo de una biblioteca Stalker | `sessions[{path, session_id, status, archived}]`, `pending[]`. |
| `handoff ASSET…` | Intercambio portable `creator-media-handoff/1` | Ver [media-library-v1](media-library-v1.md). |
| `resolve ASSET` | Ubicaciones de un asset | Lento (identidad de cada volumen); preferir `sources`. |

Los comandos que mutan (`plan`, `apply`, `relayout`, `confirm-cleanup`, `deliver`, `restore`) sólo los inicia el usuario con un plan revisado; otra app no los llama sin esa revisión. Un campo nuevo opcional no rompe a nadie. Quitar o cambiar el significado de un campo exige nueva versión y nota aquí.

## Cómo actualizan este contrato otras sesiones

Una sesión de Stalker o Clipper **puede editar `docs/contracts/*.md` de este repo**, y nada más: ni código, ni pruebas, ni STATUS/PLAN.
- Actualiza su fila de adopción, con fecha y commit de su repo.
- Si toma o descubre una decisión que afecta a las tres apps (nombres, identidad, rutas, estados, señales), la registra en el contrato correspondiente con fecha, repo que la origina y compatibilidad. No reinterpreta en silencio.
- Si necesita un cambio en el productor (un campo, un comando, un bug), lo agrega a «Pendientes para n-creator-utils» en vez de editar código de este repo.
- Los textos de los prompts vigentes están en [next-sessions.md](next-sessions.md).

### Pendientes para n-creator-utils

| Fecha | Origen | Pedido | Estado |
| --- | --- | --- | --- |
| — | — | (vacío) | — |

## Otros consumidores

**Atlas:** «Revisa el modelo de asset/ubicación como consumidor. Conserva pares JPG+DNG, coverage exports y nombres de captura. Propón un adaptador que resuelva raíces por IDs/manifiesto sin renombrar entradas existentes, y diferencie estado humano persistente de derivados regenerables. El media manager no debe redefinir GPS, vuelo, quality o fotogrametría.»
