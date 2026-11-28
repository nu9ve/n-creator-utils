# Quién llama a `nu9ve.py media`

Estado: **inventario del 2026-11-27**, paso (a) de F5 en [PLAN](../../PLAN.md#tras-la-semana-de-uso-pedido-del-usuario-2026-11-26) («retirar Python de `media/`»). Es sólo lectura: no cambia `nu9ve.py` ni a ningún consumidor. Se leyó en Clipper `f16a8ca3`, Kyū `cd6d1c8`, Stalker `1d38e1b` y en este repo. El paso (c) ya reenvía a Rust; el aviso para la sesión de Clipper está [abajo](#aviso-para-la-sesión-de-clipper).

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

## Paso (c), hecho el 2026-11-27

`nu9ve.py media <orden>` reenvía a Rust las órdenes de lectura de arriba (`projects`, `sources`, `volumes`, `resolve`, `captures`, `integration-capabilities` y `retitle-operations`). Responde lo que respondería `nuun-engine media`: los mismos valores que Python, con las claves ordenadas ([nu9ve.py](../../nu9ve.py), `rust_media`).

- **Qué motor:** sólo `NUUN_ENGINE` si está puesto. Si no, en este orden: el `nuun-engine` junto a `backend/source/nu9ve.py` dentro de la app, las builds `release` y `debug` de este checkout, y la app instalada (`~/Applications` y `/Applications`).
- **Cuándo responde Python:** con cualquier otra orden (`rpc`, `rpc-serve`, escrituras, `--help`, `--state-dir=…`), sin ningún motor, con `NUUN_MEDIA_ENGINE=python`, o cuando el motor sale con 2. Un 2 es un error de uso, o un motor anterior a (b), como el de la app instalada hoy (`f65bb6e27f56`) o una build `release` vieja; Python da entonces su respuesta y su uso de siempre.
- **Pruebas:** `ForwardTest` en [test_media_cli_rust.py](../../tests/test_media_cli_rust.py) comprueba que el reenvío da los bytes de Rust, que un motor sin la CLI deja la respuesta a Python, que los errores de uso siguen saliendo con 2, y que las demás órdenes no se reenvían. La comparación con Python fija `NUUN_MEDIA_ENGINE=python`.
- **Clipper y Kyū no cambian:** Clipper, que por defecto lanza `python3 …/nu9ve.py`, ya recibe la respuesta de Rust. Kyū sigue en `nuun_unavailable` mientras no tenga un comando configurado.

### Aviso para la sesión de Clipper

Texto para que el usuario lo pase; esta sesión no escribe a otras.

> nüün, 2026-11-27: desde el commit de F5 (c), `nu9ve.py media projects | sources | volumes list | resolve` responde con `nuun-engine media` cuando hay un motor que la entiende, y si no, con Python como antes. El sobre es el mismo (`ok`, `command`, `data` o `error`) y los valores también. Cambian el orden de las claves (Rust las ordena) y el tiempo de arranque (unos 50 ms en vez de 80). Para fijar un motor, `NUUN_ENGINE=<ruta>`; para volver a Python, `NUUN_MEDIA_ENGINE=python`. Nada cambia en `CLIPPER_NU9VE` ni en `CLIPPER_NU9VE_STATE_DIR`.

## Paso (d), hecho el 2026-11-27

`nuun-engine [--state-dir D] media [--state-dir D] mcp` es el servidor MCP de `media/mcp.py` en Rust ([mcp.rs](../../engine/nuun-engine/src/mcp.rs)): stdio, las mismas ocho herramientas de sólo lectura, el mismo `serverInfo` y las mismas respuestas. Sólo llama a esos ocho métodos de lectura. `nu9ve.py media mcp` comprueba que el motor lo entienda con una conversación vacía y le cede el proceso (`exec`); con un motor viejo, o con `NUUN_MEDIA_ENGINE=python`, sigue Python.

[test_media_mcp_rust.py](../../tests/test_media_mcp_rust.py) manda la misma conversación a los dos servidores sobre dos catálogos del golden: `initialize`, cada herramienta, argumentos que faltan o sobran, una herramienta que no existe, notificaciones, métodos desconocidos y una línea que no es JSON. Las respuestas son iguales salvo los bytes libres del disco y la hora de medición, que cambian entre las dos corridas. Nadie configura este MCP hoy (arriba); el del core (`nu9ve mcp`) sigue siendo el de la suite.

## Paso (e), hecho el 2026-11-27

La app de Mac ya no lanza Python. `PersistentReader` habla sólo con `nuun-engine rpc-serve`, y una lectura de un solo uso que el servidor no guarda (`engine_info.once`, como `resolve`) va a un `rpc` propio; una escritura nunca sale del lector. `BackendClient` manda cada llamada a `nuun-engine rpc`, y lo que el motor rechaza es la respuesta. Ajustes → Diagnóstico ya no tiene el campo de Python ni el selector «Motor de media»: muestra la ruta del motor y, al compilar desde el código, deja elegirla. El motor se busca en `NUUN_ENGINE`, en el bundle, en `enginePath` de las preferencias o en `engine/target/{release,debug}` junto a `nu9ve.py`.

Antes de quitarlo se comprobó que Rust declare los 93 métodos del servicio de Python, que todos los que nombra la app estén entre los que declara, y que los `not_ported` que quedan en Rust no se den en Mac: falta de backend de volúmenes, liberación de Stalker sin Unix, finales sin la exclusión de Clipper y cierres por la liberación simple, que la app manda a `closure_apply`. El bundle todavía lleva el runtime de Python y `media/`; se quitan en (g).

## Paso (f), hecho el 2026-11-27

El golden ya estaba en git (`tests/fixtures/engine-golden/recipe.json`) y las diferenciales de Rust lo leen sin Python. Lo que todavía corría Python en vivo eran las pruebas de la CLI y del MCP: ahora las respuestas de Python quedan congeladas en [media-answers.json](../../tests/fixtures/engine-golden/media-answers.json), escritas por [media_answers.py](../../scripts/media_answers.py), y `test_media_cli_rust.py` y `test_media_mcp_rust.py` comparan a Rust con ese archivo. Los catálogos se arman en una carpeta fija para que las rutas sean las mismas en cada corrida. Se dejan fuera lo que depende de la Mac o cambia entre corridas (tamaño y uso de su disco, la hora de medición) y `integration-capabilities`, que dice qué hay instalado; de ésta sólo se prueba que Rust responda. `media_answers.py --check` entra al gate y a la CI junto a `engine_golden.py --check`, mientras exista `media/`.

Siguen corriendo Python en vivo, y se van con `media/` en (g): las pruebas de reenvío de `nu9ve.py` (la vuelta a Python), `test_engine_examples.py` del lado de Python, la comparación de `storage_collections` en `test_agent_storage.py`, y las herramientas `engine_shadow.py`, `engine_bench.py` (caso Python) y `sandbox.py --engine python`.

## Lo que queda

- **Cómo lo encuentran sin el checkout:** un `nu9ve` instalado que apunte a `nuun.app/Contents/Resources/backend/nuun-engine` cubriría a Clipper y a Kyū sin cambiarlos; la otra opción es que cada uno busque la app, como Stalker. Lo decide el usuario con la sesión de Clipper. Hasta entonces, Clipper usa el checkout y por él llega a Rust.
- **(g):** borrar `media/*.py`, el runtime de Python del bundle y las pruebas y herramientas que corren Python en vivo (lista arriba); `engine_golden.py --check` y `media_answers.py --check` salen del gate con ellos ([PLAN](../../PLAN.md#tras-la-semana-de-uso-pedido-del-usuario-2026-11-26)).
