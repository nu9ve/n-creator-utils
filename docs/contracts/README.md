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

## Tareas listas para pasar a los otros repos

No se enviaron mensajes a otros chats ni se editaron esos repos. Textos de trabajo sugeridos:

**Stalker:** «Lee `n-creator-utils/docs/contracts/media-naming-v1.md` (sección Adopción: Stalker) y `media-library-v1.md`. Mantén `stalker-clipper/1` como autoridad de captura. Implementa, detrás de una opción: proyecto/país opcionales en la sesión y nombre de carpeta `YYMONDD-HHMMSS_STALKER_MM-SS_PAIS-PROYECTO[].stalker` (inicio local de `createdAt`, duración final), renombrando al finalizar y antes de `completed`. Los internos del bundle no cambian. Agrega biblioteca durable configurable fuera de Downloads y señal estable de finalización. Prueba con la fixture del contrato (`createdAt 2026-10-25T02:58:50Z`, duración 227.6 s, MX, debloateando-mi-tele → `26OCT24-195850_STALKER_03-48_MX-DEBLOATEANDO-MI-TELE[].stalker` en America/Hermosillo). No cambies reloj ni pistas.»

**Clipper:** «Lee `n-creator-utils/docs/contracts/media-naming-v1.md` (Adopción: Clipper) y `media-library-v1.md`. La carpeta de trabajo de un proyecto (`/Volumes/ESSD/001_proyectos/e64_raw/<proyecto>/`) tiene clips de cámara y bundles `*_STALKER_*.stalker` intercalados por hora: el selector debe respetar ese orden y reconocer bundles por `manifest.json`, no por el patrón `Stalker_…`. Guarda `sessionID`/`asset_id` además de rutas. Antes de admitir migración automática, implementa un relink de sesión que actualice items, `capture_sessions.root`, pistas, voz, sync refs, artefactos y snapshots dependientes con revisión esperada y validación de hashes. Agrega selección de recordings disponibles por biblioteca/proyecto manteniendo pegado directo; orden de selección y creative profile son hints. Devuelve ACK y limitaciones; no cambies el contrato editorial. Prueba el cambio de raíz con el origen presente y ausente y conserva proyectos personales.»

**Atlas:** «Revisa el modelo de asset/ubicación como consumidor. Conserva pares JPG+DNG, coverage exports y nombres de captura. Propón un adaptador que resuelva raíces por IDs/manifiesto sin renombrar entradas existentes, y diferencie estado humano persistente de derivados regenerables. El media manager no debe redefinir GPS, vuelo, quality o fotogrametría.»
