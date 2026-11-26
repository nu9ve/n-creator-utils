# Informe para el core y nove · nüün, discos y conversaciones (2026-11-23)

Sesión en la M1. Core leído en `~/code/ai/core` (al día), nove en `~/code/ai/nove` (al día). Diseño: [suite-disks-flows](features/suite-disks-flows.md).

## Commits (en `master`, empujados)

| Commit | Qué |
| --- | --- |
| `6a84113` | Copias verificadas entre discos de trabajo: `volume_apps`, `work_copy_plan`, `work_copy_apply`, `work_copies`; recibo `nuun-work-copy/1` ([contrato](contracts/suite-work-copy-v1.md)) |
| `1c10586` | Conversaciones por dispositivo: `conversations_settings`, `_status`, `_backup_plan`, `_backup_apply`, `_run`; `living_plan` con `include` ([contrato](contracts/suite-conversations-v1.md)) |

Después, en la misma sesión: `suite_suggestions` y `suggestions_dismiss` (sugerencias al conectar, diseño §3), y las seis decisiones anotadas como tomadas.

## Qué se verificó y cómo

- `cargo test --workspace` (TMPDIR físico, `TZ=America/Phoenix`), `fmt`, Clippy `-D warnings`, `check-boundary.sh`, `engine_golden.py --check`, `tests.test_agent_storage`. 5 pruebas nuevas con discos y `HOME` de mentira.
- QA de punta a punta por `nuun-engine rpc` en una raíz aislada (`NU9VE_*`, `HOME`, catálogo propios) con imágenes exFAT/HFS+ montadas: detalle en [STATUS](STATUS.md#discos-de-trabajo-y-conversaciones--2026-11-23). No se tocó el agente instalado, el llavero, los LaunchAgents, discos reales ni el sandbox de otra sesión.

## Qué falló

- `writes_go_only_through_the_one_shot_rpc` falla con el `TMPDIR` por defecto (enlace en `/var`), también sin estos cambios: ya conocido.
- `sync-core.sh --check` avisa que el core avanzó tras `1d11d30` (Nu9veScore); no afecta. Volver a fijar queda como cambio propio.

## Lo que nove debe saber

1. **Copias entre discos:** `.nu9ve/apps/nuun/copies/<id>.json` (`nuun-work-copy/1`) en origen y destino: cada archivo con `source_path`, `path`, tamaño, `mtime_ns` y SHA-256 verificado; copia exacta de los archivos de nove del origen en `.nu9ve/apps/nuun/copies/<id>/source/nove/`. Las rutas se conservan, así que `volume_path` es el mismo. nüün nunca escribe `.nu9ve/apps/nove/`. Pasos para adoptar sin hashear en el contrato.
2. **Conversaciones:** `<base>/<colección>/<device_id>/…` con `<base>` = `004_datos/conversaciones`; en el archivo `_nuun/manifest.json` y `_nuun/receipts/*.json`; en la copia de trabajo `_nuun/working.json`. `_nuun/` no es dato. `origin_key` propuesto: `nuun:conversations:<colección>:<device_id>`.

## Para el core

- Nada roto. Sugerencia: un validador de catálogo genérico por app en Rust (nüün tiene el suyo en `suite_volume/index.rs`, ahora parametrizado por app) junto a `index_problem`.

## Pendiente (siguiente sesión de nüün)

- Hecho después: la ventana (sugerencias en Inicio, «Copiar al otro disco…», Ajustes → Conversaciones, respaldo automático con la app abierta), aceptada en el sandbox. Falta instalarla y usarla con los discos reales.
- Proyecto de Clipper con dependencias en `work_copy`; resumir copias en el índice de nüün; prueba física VIAJE/EDICION.

## Decisiones del usuario

**Tomadas por el usuario el 2026-11-23**, las seis como se propusieron ([diseño](features/suite-disks-flows.md#decisiones-del-usuario)): carpeta base en MEDIA, SQLite de Codex fuera, «sugerir cada 7 días», destino de la copia de trabajo a elegir en Ajustes, MEDIA→trabajo fuera de `work_copy`, recibo también en el origen.
