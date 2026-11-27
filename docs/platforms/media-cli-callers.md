# Quién llama a `nu9ve.py media`

Estado: **inventario del 2026-11-27**, paso (a) de F5 en [PLAN](../../PLAN.md#tras-la-semana-de-uso-pedido-del-usuario-2026-11-26) («retirar Python de `media/`»). Es sólo lectura: no cambia `nu9ve.py` ni a ningún consumidor. Se leyó en Clipper `f16a8ca3`, Kyū `cd6d1c8`, Stalker `1d38e1b` y en este repo. Antes de cambiar `nu9ve.py`, el paso (c), hay que avisar a la sesión de Clipper.

## Fuera de este repo

| Quién | Cómo encuentra a nüün | Qué pide | Qué espera |
| --- | --- | --- | --- |
| **Clipper**, el motor (`core/src/rpc_writes/handoff.rs`, `run_media`) | `CLIPPER_NU9VE` (una línea de órdenes, sin comillas), si no `nu9ve` en el `PATH`, si no `python3 /Users/nu9ve/code/nu9ve/n-creator-utils/nu9ve.py`. `--state-dir` desde `CLIPPER_NU9VE_STATE_DIR`. Plazo de 120 s; mata el grupo del proceso | `media sources --project <p>` (al importar), `media volumes list` y `media resolve <asset_id>` (en el *handoff*, para cada asset) | El sobre `{ok, data}` o `{ok: false, error: {code, message}}` por stdout. Lee sólo `ok`, `data` y `error`; ignora `command` y la sangría. Respuesta de hasta 64 MiB |
| **Clipper**, Python (`src/clipper_tui/media_library.py`, `media_handoff.py`) | Igual, con `shlex` para `CLIPPER_NU9VE` | `media projects`, `media sources --project <p>`, `media volumes list` y `media resolve <asset_id>` | El mismo sobre |
| **Kyū** (`core/crates/kyu-core/src/nuun.rs`, `service.rs`) | `KYU_NUUN_COMMAND` (separado por espacios), si no Ajustes → Integraciones («Comando de nüün», `nuun.command`), si no `nu9ve` en el `PATH`; si no hay nada, `nuun_unavailable` | `media projects`, `media sources --project <p>` y `media sources --project <p> --purpose <edit\|archive>` | El sobre, y de él sólo `data` |
| **Stalker** (`Sources/StalkerCore/NuunBackend.swift`) | `NUUN_ENGINE`, si no la app por su identificador (`app.nu9ve.media`), si no `~/Applications/nuun.app` o `/Applications/nuun.app` (`Contents/Resources/backend/nuun-engine`). Sólo en desarrollo cae al checkout `~/code/nu9ve/n-creator-utils/nu9ve.py` | Ya habla con **`nuun-engine rpc`**: `captures`, `projects`, `integration_capabilities`, `retitle_operations` y `retitle_capture`, una petición JSON por stdin | `{ok, data\|error}` |
| **MCP `media mcp`** (`media/mcp.py`) | Ningún cliente lo tiene configurado en esta Mac: ni `~/.claude.json`, ni Claude Desktop, ni Codex. El core lo nombra como el servidor que reemplaza `nu9ve mcp` ([suite-mcp](../../vendor/nu9ve-core/docs/suite-mcp.md)) | — | — |
| nove, neuf y curado | No llaman a nüün. nove lee los índices que nüün publica en los discos (`.nu9ve/apps/nuun/index.json`) | — | — |

`nu9ve` en el `PATH` no existe hoy: es un alias de zsh (`python3 ~/code/nu9ve/n-creator-utils/nu9ve.py`), que un proceso lanzado por otra app no ve. Por eso, sin configurar nada, Clipper usa la ruta del checkout y Kyū dice `nuun_unavailable`.

## Dentro de este repo

- **La app**, como respaldo de Rust: `PersistentReader` lanza `nu9ve.py media rpc-serve` y `BackendClient`, `nu9ve.py media rpc` ([Protocol.swift](../../native/Nuun/Sources/NuunCore/Protocol.swift)). `build_media_app.py` copia `nu9ve.py` y `media/` al bundle (`backend/source`).
- **El gate y las herramientas:** `engine_golden.py` (el oráculo), `engine_shadow.py` (la sombra), `engine_bench.py` y `sandbox.py` con `--engine python`.
- **`legacy-save` y descargas**, que el paso (g) conserva.

## Qué cubre ya Rust

Las órdenes de fuera se resuelven en métodos que `nuun-engine` ya sirve: `projects`, `sources` (con `purpose`), `volumes`, `resolve` (con `revision`) y `captures`. El golden (`engine_golden.py`) compara `projects`, `resolve` y las fuentes del editor. `volumes` no está entre sus lecturas; lo compara la prueba de la CLI (abajo). Hasta el paso (b), `nuun-engine` sólo entendía `rpc`, `rpc-serve` y `suite-frames`, y los consumidores mandan `media <orden> --opción`.

## Paso (b), hecho el 2026-11-27

`nuun-engine [--state-dir D] media [--state-dir D] <orden>` responde las órdenes de lectura que mandan los de fuera: `projects`, `sources --project P [--purpose edit|archive]`, `volumes list`, `resolve ASSET [--revision R]`, `captures ROOT`, `integration-capabilities` y `retitle-operations` ([main.rs](../../engine/nuun-engine/src/main.rs), `media_cli`).

- **Sobre:** `{"ok": true, "command", "data"}` con sangría de 2, o `{"ok": false, "error": {code, message}}` en una línea con salida 1. Un error de uso sale con 2 y escribe el uso en stderr, como argparse. Las claves salen ordenadas, no en el orden de Python; ningún consumidor lo lee.
- **Igual a Python:** [`test_media_cli_rust.py`](../../tests/test_media_cli_rust.py) corre cada orden con los dos motores sobre dos catálogos del golden (`reads` y `projects`, este con fuentes del editor y tres discos) y compara valores y salida. Cada orden responde con datos al menos una vez.
- **Diferencia que queda:** Python rechaza un `--state-dir` con un enlace simbólico en la ruta (`unsafe_state`) y Rust no.
- **Escrituras:** ninguna orden de escritura (`apply`, `retitle-capture`…) pasa por aquí. Siguen en `rpc`.

## Lo que queda antes de (c)

- **Cómo lo encuentran:** con la CLI ya en Rust, `nu9ve` puede ser un enlace instalado a `nuun.app/Contents/Resources/backend/nuun-engine`, lo que cubre a Clipper y a Kyū sin cambiarlos. O cada uno sigue a Stalker y busca la app. Lo decide el usuario con la sesión de Clipper, antes del paso (c).
- **Avisar a Clipper** antes de (c): su ruta por defecto es el checkout de `nu9ve.py`, que (c) convierte en un reenvío a Rust.
