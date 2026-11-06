# Compact project context

Snapshot: 2026-11-06, America/Hermosillo. Start here, then [STATUS](STATUS.md) and [PLAN](../PLAN.md). This is a handoff, not a claim of release readiness.

## Product and user workflow

nüün is the macOS app in n-creator-utils, envisioned as a multitool for creative workflows. Its most developed area today is media: SD ingest, verified archive, working copies, discovery/catalog and retrieval. Stalker captures; Clipper edits/syncs/exports; Final Cut finishes manual edits; Atlas consumes drone images. Downloads/stems remain separate utility arms; the old embedded Clipper was removed. MIDI code is experimental; lighting, IoT and Raspberry Pi connections are future directions, not integrated app features.

Typical video flow: SD → MEDIA archive + ESSD working raw → Clipper → Final Cut → finals → published → MEDIA finals. Audio normally needs MEDIA + SSD1. Projects/profiles can be mixed on one card; project/country/city/profile are independent fields. Preserve selection order, sidecars and complete Stalker bundles.

Existing disk layout should remain useful: ESSD `001_proyectos/e64_raw`, `001_proyectos/e64`, `002_finales`; MEDIA502 `001_raw`, `002_finales`, `003_curado`. Do not reorganize old TBs to introduce the catalog. Named layout (plan schema 2, [layout.py](../media/layout.py), decided at plan time and part of the digest) restores the historical names `YYMONDD-HHMMSS_MODEL_MM-SS_COUNTRY-PROJECT[].EXT` and MEDIA type/year/month folders; Sony XML goes to `metadata/sony/`, camera indexes/thumbnails keep their tree under `metadata/sony/support/`. Work copies are flat in `<profile>_raw/<project>/` with only editable media and whole Stalker bundles, so a folder lists in capture order. Copies land as `.name.ncpartial`, are verified, then renamed without replacing anything; identical existing files are adopted, different ones block the plan. Schema-1 plans keep per-job containers. Stalker bundle folders take the same dated name (contents untouched); `media relayout` renames earlier copies in place. Contract: [naming v1](contracts/media-naming-v1.md). Single archive replica accepted by the user; further redundancy is deferred: [replication policy](features/replication-policy.md).

## Product decisions to preserve

- `save` should finish with the SD free for new projects. Copy → verify all effective required destinations → journaled cleanup. Copy-only is optional/diagnostic, not the intended default for a card.
- If MEDIA exists but ESSD/SSD1 is absent: explicitly confirm archive-only for that operation; record work pending or deliberately skipped. Later copy from archive, without the old SD. Do not silently make this exception the profile default.
- No registered archive/work destination online: SD insertion is silent, with no deep automatic scan. Manual inspect is still allowed.
- A completed backup, work readiness, publication and source readiness are different states. Never infer two replicas from two folders on one physical disk.
- Sony (`sony_clips_then_format`): after verification the engine removes clips, thumbnails and DCIM stills from the card but never its indexes (`AVF_INFO`, `PRIVATE/SONY`, `M4ROOT` databases, AVCHD index); the job ends `camera_action_required` until the camera format, confirmed by `confirm-cleanup` (indexes must have been rewritten). DJI drone/360/action ambiguity requires an explicit category. No persistent device/model registry yet.
- Stalker must be completed; originals/raw microphone/events stay together. Work with active recording must not copy an incomplete session. A reviewed ingest plan verifies required archive/work copies, then defaults to a journaled local release in the same apply flow; it asks Clipper CLI to relink to a verified copy, checks ACK/dependencies and retains the source if any gate fails. Ajustes → Capturas can opt out and keep the original for later release from Inicio or Discos. Full SQLite snapshots protect human metadata. [Workflow and limits](operations.md#bandeja-stalker-proyectos-y-liberar-espacio).
- No silent media staging on the internal drive. Local catalog/journal metadata is expected; local media destinations require explicit registration/choice.

Cierre de proyectos (primera entrega, 2026-11-04): conservar fuentes y finales; descartar `renders/` identificados sólo mediante plan y consentimiento explícitos. Proyectos vincula finales y declara entregas completas; el cierre reutiliza recibos, Transfer y relink/ACK existentes. Editables Clipper se conservan en nüün; Clipper ya ofrece retiro editorial con desregistro por CLI/TUI, decidido desde Clipper y observado mediante deps frescos. No autoriza borrar media. Final Cut queda fuera. [Contrato y límites](contracts/project-closeout-v1.md).

Ingreso vigente (2026-11-05): Entrada → Flujo → Resultado, proyecto general como fallback, auxiliares ligados al ingreso y filas por bundle. Un botón autoriza el plan mostrado y la limpieza verificada; excepciones inline. Publicación = `subidos` o marca manual por final, sin canales. «Respaldar publicados» no termina el proyecto ni retira raws/renders. [Ingreso](contracts/ingest-flow-v1.md), [cierre](contracts/project-closeout-v1.md).

## Próxima operación y continuidad · 2026-11-06

El usuario reporta que Untitled ya se formateó y reutilizó varias veces para Sony: el incidente de montaje del 27 de octubre no es un bloqueo operativo actual. No acredita causa/reparación ni recuperación del contenido antiguo. Sí reporta un aviso recurrente de formato/limpieza pendiente. El motor Sony borra clips verificados y conserva índices; confirmar formato es un paso separado por job, sin reconciliación automática al reutilizar la SD. Falta identificar el aviso/job real; no cerrar históricos a ciegas ni quitar la guarda de índices.

Próxima aceptación acompañada: dos videos multicam recién grabados; asignaciones elegidas por el usuario desde la app. Instalada build `88fa9b79a571`: compacto flotante, modos mayores normales; popup por defecto activo y preferencia local habilitada a petición (estaba false). Usuario aceptó flotante; detección aún incierta y pendiente. Reporta ingreso Stalker correcto y Sony con FileNotFoundError al finalizar limpieza; seis retiradas constan en journal, sin causa confirmada. Empezar con ventana cerrada y proceso nüün activo en menubar, insertar SD con destinos online y revisar el plan antes de aplicar. Recorrido y evidencia pendiente en [Sony/multicam](sony-validation.md#aceptación-acompañada-sony-y-dos-multicams--2026-11-06). No sustituye gates de desconexión ni autoriza pruebas destructivas sobre material único.

El usuario hace el ingreso desde nüün. Excepción operativa explícita del 6 noviembre: pidió recuperar los dos clips Sony de `buscando-baches-con-el-dron` a ESSD con la herramienta existente; `deliver_work` completó/verificó ambos (7.03 GB), sin tocar SD ni borrar archivo. Nuevos ingresos: trabajo Todo el lote por defecto, destinos visibles, preview de slug y sugerencias de proyectos de sesión/pendientes; ver contrato de ingreso. Reporta además borrados externos de algunos intermedios y de proyectos desde Clipper; no hay inventario ni evidencia de desregistro para cada caso. Evaluar el estado actual al revisar desde la app: ausencia de renders antes de un plan no equivale a cierre; desaparición posterior a revisión exige replanificar; registro Clipper ausente/offline puede bloquear retiro. [Límites de cambios externos](contracts/project-closeout-v1.md#cambios-externos-y-borrados-previos--2026-11-06).

Atlas actualizó su [propuesta](contracts/atlas-drone-v1.md) el 6 de noviembre (A1–A10): conservar DCIM y JPG+DNG en archivo; trabajo JPG útil/DNG por demanda, no nadir separado; estado humano, exports y productos ODM costosos frente a intermedios regenerables. Nüün sin implementar; no trasladar la limpieza reportada por Atlas a evidencia propia ni cambiar su estado geoespacial.

## Code map

```text
nu9ve.py → media/save.py (human save) / media/cli.py (JSON CLI/RPC/MCP)
media/service.py → planner + volumes + transfer + catalog + library
media/common.py → safe tree-relative IO, hashes, errors, JSON
native/MediaMenu → WindowRoot (3 modes) / Model / JSON bridge / FSEvents
media/workspaces.py → creative profile registry; planner freezes profile_routes
manager/file.py → legacy-save + existing downloads/stems
```

Use [modules](modules.md), [operations](operations.md) and [development](development.md) for details. Contracts are indexed [here](contracts/README.md); avoid new direct writes to an editor's JSON.

## Current working state

The unified **nüün** window has compact/selection/full modes; source inspection is explicit, and choosing a source or mode does not deep-scan. The local development app installs at `~/Applications/nuun.app`. See [STATUS](STATUS.md) for current evidence and installation limits, [PLAN](../PLAN.md) for the next gates, and [window flow and ownership](features/nuun-window.md) for details. Check Git status and HEAD before resuming. Automated tests never use MEDIA/ESSD/SDs; any live backup is an explicit, user-requested operational run with a reviewed plan.
