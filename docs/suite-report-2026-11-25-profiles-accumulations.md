# Informe para el core y nove · nüün, perfiles, acumulados y benchmarks (2026-11-25)

Sesión en la M1. Core en `~/code/ai/core` al día (`git pull --ff-only`, `f267022`). nove leído en `~/code/ai/nove` (`889d8ba`). Detalle y evidencia: [STATUS](STATUS.md#perfiles-y-acumulados--2026-11-25). Diseño de acumulados: [storage-map](features/storage-map.md#acumulados).

## Commits

**Ninguno.** Nadie pidió hacer commit ni push. Todo está sin commit en `master`, sobre `be4d0e4`. Al hacer commit conviene separar dos cambios:

1. **Volver a fijar el core**, un cambio propio: `vendor/nu9ve-core.paths`, `vendor/nu9ve-core/**`. De `1d11d30` a `f267022`, con `rust/nu9ve-profiles`, `rust/nu9ve-bench`, `scripts/nu9ve_profiles.py`, `profile-examples.py`, `nu9ve_bench.py`, `bench-examples.py` y `bench-from-atlas.py`, que necesita el `--check` de bench.
2. **Perfiles, acumulados, U1 y bench**: lo demás.

## Qué se hizo

| Pieza | Dónde |
| --- | --- |
| `suite_profiles` (lectura) y `suite_profiles_register` (escritura con `confirm`): sólo la clave `nuun`, con alias si el perfil ya existe | `engine/crates/nuun-media/src/suite_profiles.rs`; Ajustes → Perfiles → «Perfiles en esta Mac» |
| Registro del motor `nuun`: `library` (su catálogo) y `profile_id` (el de omisión, sólo si ya tiene su entrada `nuun`), con la versión menor de `nu9ve-registry-v1` | `engine/nuun-engine/src/suite.rs` |
| Acumulados, fase 1: reglas como datos, inventario, plan, volúmenes y revisiones | `accumulations.rs`, `accumulations/rules.json`, `scripts/suite_storage.py`, Discos → Acumulados… |
| Dataset `storage-accumulations` (`table`, `personal`, `copied`, con `revision` y hash) en `data_catalog`, y su NDJSON en `data_export` (`ready`) | `suite_volume/catalog.rs` |
| U1 de nove: `conversations_export_working_v1` en el socket, con `conversations_export_working` (sólo lo decidido, en segundo plano, con `run_id`) y `conversations_export_status` | `suite.rs`, `conversations::export_decided` |
| Benchmarks nu9ve-bench-v1: golden Python ↔ Rust con `response_equal`, recorrido (archivos/s) y soak con `memory_slope_bytes_per_iter` | `scripts/engine_bench.py` → `bench/nuun.ndjson` (ignorado por git) |

## Qué se verificó y cómo

Todo con fixtures, raíces temporales, `--state-dir`, `NU9VE_PROFILES_FILE`, `NU9VE_DEVICE_FILE`, `NU9VE_ENGINES_DIR` y un `HOME` de fixture. No se tocaron `~/Library/Application Support/nu9ve/profiles.json` (no existe en la M1), `~/code`, la biblioteca real, los discos, el agente ni los LaunchAgents.

- **Rust:** `cargo fmt --check` y Clippy `-D warnings` limpios. `cargo test`: 191 pruebas y 23 diferenciales (`TZ=America/Hermosillo`, `TMPDIR` físico). Hay 9 pruebas nuevas: 4 de perfiles, 5 de acumulados (una con `data_catalog`/`data_export`) y 1 de U1.
- **Python:** `unittest discover`, 420 pruebas. Las 5 nuevas de `tests/test_suite_storage.py` cubren: sin archivo; archivo de nove con `e64`, al que sólo se agrega `nuun`; dos escritores a la vez (el motor de Rust y `nu9ve_profiles.py` en un hilo, 20 escrituras); un archivo inválido que no se toca; y el escaneo de acumulados por RPC. `engine_golden.py --check` está al día.
- **Swift:** `swift test`, 32 pruebas. La app compila con las vistas nuevas, pero no se abrió en ventana ni se instaló.
- **Core:** `sync-core.sh --check` («is current»), `profile-examples.py --check`, `bench-examples.py --check` y `check-boundary.sh` pasan.
- **Bench** en la M1, sobre un registro temporal:

  | Caso | Python | Rust |
  | --- | --- | --- |
  | Lecturas del dorado | Unos 60 ms por proceso | 5 a 11 ms, `response_equal=True` |
  | Recorrido | `storage_scan`, unos 8 400 archivos/s | Acumulados, unos 146 000 archivos/s |
  | Soak de 2 000 lecturas | — | 0 B/iter (límite 4 096) |

## Qué falló y si ya fallaba

- `scripts/check_localization.py` da 111 claves sin inglés. **Ya fallaba** con claves de entregas anteriores (Adoptar disco, Handycam…), y esta entrega suma las suyas. No está en `checks.yml`.
- El primer soak usaba `snapshot` y daba 3 iter/s, porque `snapshot` pregunta por los discos montados con diskutil. Se cambió a lecturas del catálogo. No es un fallo del motor, pero `snapshot` no sirve para un soak sobre fixture.

## Lo que nove debe saber

- **Perfiles:** nüün lee `profiles.json` y sólo escribe su clave `nuun`. `e64` lleva `aliases: ["erikiado64"]` si nove ya lo creó como «Erik». La biblioteca `nuun` de todos sus perfiles es el mismo catálogo.
- **Acumulados:** el dataset `storage-accumulations` es una tabla NDJSON, una fila por hallazgo. Campos: `scan_id`, `scanned_at`, `path`, `rule`, `kind`, `bytes_on_disk`, `bytes_logical`, `files`, `last_used_at`, `repo`, `repo_activity_at`, `agent_activity_at`, `owner_app`, `owner_profile`, `regenerates`, `cost`, `risk`, `action`, `confirmations`, `volume` y `git`. Cada escaneo es una revisión nueva (`revision.id`), y el archivo exportado nunca se reescribe. Las rutas son personales.
- **U1:** el socket anuncia `conversations_export_working_v1`. Una petición sin parámetros exporta lo decidido. Cualquier otro destino o colección da `confirmation_required`. La respuesta es `{"status":"started","run_id"}`, y `conversations_export_status {run_id}` responde `running`, `done` (con `result`) o `failed`. Sólo funciona mientras la app de nüün tiene abierto su motor. El formato de `working.json` no cambió: el contrato de compatibilidad sigue vigente.
- **Bench:** `bench/nuun.ndjson` con `suite: nuun-engine`, casos `golden/<método>`, `walk/storage-index`, `walk/accumulations` y `soak/catalog-reads`, y `subject.commit` de 12 caracteres.

## Decisiones abiertas

1. **Umbral del disco interno:** 20 GiB por omisión (configurable en la vista y por CLI). ¿Otro valor? En la M1 hubo 169 MiB libres.
2. **Qué va a la Papelera y qué no** en la fase 2. Hoy `git` y Docker son `manual`: se liberan con su herramienta (`git gc`, `docker system prune`). Lo demás va a la Papelera, con dos confirmaciones para los respaldos de apps. ¿Borrar `node_modules` y `target/` en definitiva, sin Papelera, si la persona lo confirma?
3. **`e64` y la cuenta de Fractos:** `account_link` sigue en `null`. Lo decide F2 en el core.
4. **Tipo de `nu9ve-xyz`:** se crea como `team` si no existe. ¿Es una persona o una marca?
5. **Sesiones de agentes:** hoy se toma el mtime de `~/.claude/projects/<ruta>`. Para usar `agents:summary:sessions` de nove, nüün necesita un cliente de otros motores registrados, que no tiene. ¿Lo trae el core (`nu9ve-ipc`), o nove expone algo por el agente?
6. **Copias en el bench:** falta un caso de copia (GiB/s) sobre fixture que no exija discos con marcador.
7. **Bibliotecas en el mapa:** se muestran con su ruta, pero no se miden. ¿Medir su tamaño, que es caro en la biblioteca de nove?
