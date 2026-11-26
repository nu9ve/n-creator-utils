# Qué se extrae a `nu9ve/core` y cómo lo adopta cada app

Plan vivo desde el 2026-11-16. Ordena en olas las piezas que se repiten entre apps, dice qué toma cada app y guarda el inventario de lo que cada una repite. Qué contrato cumple cada app está en [adopción por app](adoption.md); el mapa de apps, en [la suite](suite.md); la mudanza de nüün archivo por archivo, en [qué se muda](suite-split.md). Las reglas de siempre: cada pieza entra en un cambio propio aquí, con su prueba y su línea en el CHANGELOG, y la app de origen deja su primera copia fijada en el mismo paso ([AGENTS](../AGENTS.md)).

## Hecho

| Pieza | Fecha | Quién la usa |
| --- | --- | --- |
| Contratos, datos y documentos de la suite | 2026-11-16 | Todas las apps con copia fijada |
| `sync-core.sh` con `--check`, `--strict` y `--from` | 2026-11-16 | nüün, Fractos, nove, kyu, SoundBender, curado, Clipper |
| `NuMacShell` en `swift/` (macOS 15, [por qué](../swift/README.md#macos-mínimo-15)) | 2026-11-16 | Fractos, Kyū (`77a3449`) y SoundBender (ventana principal en `ShellWindowHost`, `1cf31e3`). Lo siguen nüün, tras su semana de uso diario, y las demás apps de Mac |
| [`nu9ve-json-v1`](../contracts/nu9ve-json-v1.md), sus vectores y `rust/nu9ve-json` | 2026-11-16 | nüün (motor de Rust) y Clipper (sobre, `sha256_hex`, `uuid4`) |
| [`nu9ve-fs-v1`](../contracts/nu9ve-fs-v1.md) y `rust/nu9ve-fs` | 2026-11-16 | nüün (`nu9ve-volumes`), Clipper (`fsio.rs` y `store.rs`) y curado (exportación XML, `2fcc0c0`) |
| [`nu9ve-engine-v1`](../contracts/nu9ve-engine-v1.md) (propuesta), sus [ejemplos](../data/engine-examples-v1.json) y `scripts/engine-examples.py` | 2026-11-16 | Clipper y nüün pasan los 20 casos en sus dos motores. nove, pasos 1 a 3 en Mac; falta probarlo en Windows ([§12](../contracts/nu9ve-engine-v1.md#12-diferencias-por-app-y-camino-a-v1)) |
| [`nu9ve-engine`](../rust/nu9ve-engine/): servidor de líneas y marcos con lectura acotada, ids exactos y `engine_info` | 2026-11-16 | Clipper (`clipper-core serve`), app de origen, y nüün (`nuun-engine rpc-serve`) |
| `scripts/check-boundary.sh`: nada en `rust/`, `swift/` o `windows/` nombra una app; con carpetas como argumento, revisa las de una app | 2026-11-16 | Gate de aquí y de nüün (sus `nu9ve-*` que aún no se mudan), que dejó su `check_suite_boundary.sh` |
| Vectores de tema probados en Swift (15 roles), C# (nüün) y TypeScript (curado, `accent` y `onAccent`) | 2026-11-16 | `Nu9veTheme`, nüün y Kyū |
| [`Nu9veTheme` en `swift/`](../swift/README.md): roles, `readable`, presets y skins | 2026-11-16 | Nüün (app de origen) y Kyū, ambos con mínimo macOS 15 |
| [`Nu9ve.Core` y `Nu9ve.WinUI` en `windows/`](../windows/README.md), sin la identidad de nüün y con sus pruebas; skins propios de una app | 2026-11-16 | nüün en Windows (app de origen, `d0cb27c`) y nove (`cee0684`: Puerto, Carta náutica y los comunes) |
| [`nu9ve-loudness`](../rust/nu9ve-loudness/) (LUFS y true peak) y sus vectores de audio | 2026-11-17 | SoundBender (`3df4dfb`, fijado en `bcd8d43`, que ya trae la versión el doble de rápida) y Clipper (mastering del master, `0bdd1f74`, fijado en `08744e3`). Falta curado |
| `PeriodWindowBar` en `NuMacShell`: la barra de época de los comunes | 2026-11-18 | SoundBender (`2ab211d`). Kyū y nüün siguen con su copia |
| [`nu9ve-registry-v1`](../contracts/nu9ve-registry-v1.md), [`nu9ve-data-v1`](../contracts/nu9ve-data-v1.md), `rust/nu9ve-ipc` y `rust/nu9ve-mcp` con el binario `nu9ve` | 2026-11-19 | nove se registra con `nu9ve-ipc` y sirve `data_catalog` y `mcp_tools` (`d430c04`, `c7d223e`, `036315d`; su A53, A55 y A56). Ninguna app usa todavía `nu9ve mcp` con un cliente real |
| [`nu9ve-artifact-v1`](../contracts/nu9ve-artifact-v1.md) y `rust/nu9ve-artifact` | 2026-11-19 | nove exporta (`2f5468d`, su A57) y Clipper importa como overlay (`0e3056f2`). **Versión menor del 2026-11-24** (PDF en el render y `documents`): la escribe la pestaña Salidas de nove (su A95), en integración |
| [`nu9ve-bench-v1`](../contracts/nu9ve-bench-v1.md), `rust/nu9ve-bench`, el ayudante `scripts/nu9ve_bench.py` y el traductor de Atlas | 2026-11-21 | Clipper, primer usuario: fijado en `6284877` (`ad80ef2a`); su M0 escribe la línea base Python ↔ Rust del motor en el registro (`835f4fc5`, 26 corridas). **Versión menor del 2026-11-24:** corridas de sólo calidad y `nu9ve_bench::atlas` (con `evolucion.json`). nove la fija para su S10 (fuentes, `bench:runs` y `bench:quality`, su A95), en integración. **Versión menor del 2026-11-25:** `resources` y `soak` (fugas de memoria en el gate); nove la fija en su A96 con `scripts/gate-bench.py`, en integración |
| [`Nu9veFormat`](../swift/README.md) en `swift/` y `Nu9ve.Core/Format`, con [`ui-format-vectors-v1.json`](../data/ui-format-vectors-v1.json): valores, pasos, cifras, bytes, código de tiempo e instantes de [`nu9ve-controls-v1`](../contracts/nu9ve-controls-v1.md) (C1 de [controles comunes](suite-ui.md#7-orden-de-extracción)) | 2026-11-22 | Nadie todavía; nació genérica. Lo usan los controles de C5. C# sin correr en Windows |
| [`nu9ve-midi`](../rust/nu9ve-midi/) y perfil físico Pioneer DDJ-SB v1 | 2026-11-22 | Pendiente de fijar en curado, app de origen. El [handoff del crate](../rust/nu9ve-midi/README.md#fijar-y-verificar) documenta las rutas, IDs y pruebas para adoptar. |

| [`nu9ve-music`](../rust/nu9ve-music/) en `rust/`: grilla de beats, mapa de tempo, primer sonido y clave, desde curado tras compararlo con SoundBender | 2026-11-22 | Nadie todavía; lo adoptan curado (app de origen) y SoundBender desde su motor en Rust |
| [`Nu9veScore`](../swift/README.md) en `swift/`: el compositor sin conexión de Clipper y video en un solo motor, con dos revisiones que suenan como cada uno, comprobado muestra por muestra | 2026-11-22 | Nadie todavía; lo adoptan Clipper (`ClipperScore`) y video (`fractos-score`) |
| [`Nu9veVision`](../swift/README.md) en `swift/`: Apple Vision sobre una imagen (caras, cuerpos, poses, cabezas, máscara de personas recortada) | 2026-11-22 | Nadie todavía; lo adoptan video (`fractos-vision`) y Clipper (`SubjectDetection`) |
| [`nu9ve-agent-v1`](../contracts/nu9ve-agent-v1.md) y [`nu9ve-exchange-v1`](../contracts/nu9ve-exchange-v1.md), `rust/nu9ve-device`, `rust/nu9ve-exchange` y `nu9ve agent` en el binario `nu9ve`: identidad, emparejado, carpeta de intercambio, servicio de fondo, instalador, motor `suite`, pedidos, objetos y dueñas | 2026-11-22 | Nadie todavía (U7a, U7b, U7c). La primera que lo trae es nüün; nove lo usa para «otras máquinas» |
| [`nu9ve-profile-v1`](../contracts/nu9ve-profile-v1.md), `rust/nu9ve-profiles`, el ayudante `scripts/nu9ve_profiles.py` y la versión menor de [`nu9ve-registry-v1`](../contracts/nu9ve-registry-v1.md) con `profile_id` | 2026-11-25 | nove, primer escritor (su A98, en integración). nüün lo lee para saber qué perfiles hay en la máquina y en qué apps |
| [`nu9ve-device-v1`](../contracts/nu9ve-device-v1.md), [`nu9ve-volume-v1`](../contracts/nu9ve-volume-v1.md) y la versión menor de [`nu9ve-data-v1`](../contracts/nu9ve-data-v1.md) (raíces en un disco, clave `conversaciones`), con ejemplos y validadores | 2026-11-22 | Nadie todavía. Es el paso del core en F0 de [varias máquinas](suite-devices.md#10-fases); siguen nüün, después Atlas, nove y Clipper |

## Decisiones tomadas para lo que sigue

- **`nu9ve-fs`** (el usuario, el 2026-11-16):
  - La sincronización por omisión es `F_FULLFSYNC` en macOS (`File::sync_all`), porque nüün respalda media y ahí lo que importa es que los datos lleguen al disco.
  - El `fsync(2)` simple, el de Python, existe como variante explícita para los motores que comparan con Python (Clipper y las pruebas doradas de nüün).
  - Un fallo al sincronizar la carpeta **se propaga**, como en Python. Ignorarlo es justo lo que hoy hace diferir la cola de Rust de Clipper de la de Python.

- **`nu9ve-engine-v1`** (el usuario, el 2026-11-16, [§13](../contracts/nu9ve-engine-v1.md#13-decisiones-abiertas)):
  - Un imprevisto del motor es `internal_error`, no `operation_failed`.
  - En la sombra, `1` y `1.0` son respuestas distintas.
  - El registro de la sombra guarda rutas, códigos y tiempos, nunca valores.
  - En Windows, un motor se cancela primero de forma cooperativa (método o archivo, con plazo de gracia) y al final con `TerminateJobObject`. Sin `GenerateConsoleCtrlEvent`.
  - No hay progreso ni eventos dentro del canal. Un flujo de eventos es de `nu9ve-jobs`.
  - nove retira su sobre viejo en la primera versión en que todos sus clientes envían `id`.
  - El crate `nu9ve-engine` nace desde Clipper; nüün lo adopta después de cerrar sus brechas.

## Lo que sigue, por olas

### Ola 1 · ahora

| Pieza | Destino | De dónde | Primeros usuarios |
| --- | --- | --- | --- |
| Adoptar [`nu9ve-engine-v1`](../contracts/nu9ve-engine-v1.md): ~~Clipper valida `contract` en su cliente~~ (hecho, `39c6db70`); nüün corrige `id`, códigos y `reason` en Python y Rust a la vez; nove hace que sus clientes lean los dos sobres ([§12](../contracts/nu9ve-engine-v1.md#12-diferencias-por-app-y-camino-a-v1)) | Cada app, en su repo | El contrato y sus ejemplos, ya aquí | Clipper ya sirve los campos de v1; nüün sigue y nove lo adopta en cuatro pasos. Las decisiones 1 a 3 ya están tomadas: nüün cambia códigos y su sombra en el mismo paso |

### Ola 2 · con dos usuarios a la vista

| Pieza | Destino | De dónde | Condición |
| --- | --- | --- | --- |
| `nu9ve-loudness` (LUFS, limitador) y vectores de audio | `rust/` y `data/audio/` | curado `audio-engine/loudness.rs`; SoundBender `engine/soundbender-core` | **Construido el 2026-11-17** (LUFS y true peak; el limitador no entra, [audio §5](platforms/audio.md#5-curado-y-soundbender-pieza-por-pieza)). SoundBender lo usa desde el 2026-11-17 (`soundshop` `3df4dfb`) y Clipper desde el 2026-11-22 para medir el master (`0bdd1f74`). Falta que curado lo adopte. **Condición cumplida el 2026-11-16:** SoundBender decidió que su motor portable es Rust (su decisión 036, `3271f4d`) y ya portó LUFS y los limitadores con los mismos bits que Swift. Hay dos implementaciones en Rust, ya comparadas con las mismas señales (los 23 dorados de SoundBender): el LUFS de curado lee entre 0,04 y 0,29 LU bajo la norma, así que el crate parte del de SoundBender y suma el true peak de curado ([audio §5](platforms/audio.md#5-curado-y-soundbender-pieza-por-pieza)) |
| `nu9ve-volumes` | `rust/` | nüün; curado `identity.rs`/`relink.rs` como segundo | **Mudado** el 2026-11-22 con el marcador de `nu9ve-volume-v1` ([CHANGELOG](../CHANGELOG.md)) |
| Guía de prácticas comunes | `docs/practices.md` | Lo que ya repiten cinco apps: QA aislada por variable de entorno, instalar en la carpeta de apps del usuario con respaldo del anterior, gate con `sync-core.sh --check`, CHANGELOG por pieza | Escribirla con lo que ya hacen nüün, nove, curado, kyu y SoundBender, sin inventar reglas nuevas |
| `nu9ve-py`: semántica de CPython (`repr`, `round`, `json.dumps` con indentación, cadenas, `pathlib`, `datetime`) | `rust/` | Clipper `py.rs`, `pyval.rs`, `paths.rs`; nüün `nu9ve-time` | Mientras nüün y Clipper porten desde Python. Vectores generados por CPython |

| Cliente Swift de [`nu9ve-engine-v1`](../contracts/nu9ve-engine-v1.md): proceso del motor, handshake con `contract`, peticiones emparejadas por `id`, plazos, caída y reinicio con backoff, `not_ported` | `swift/` | nüün `Protocol.swift` y su cliente; nove (Swift) | **Hecho el 2026-11-22** (`f813334`): `Nu9veEngine` en [`swift/`](../swift/README.md#nu9veengine-cliente-de-nu9ve-engine-v1), con leases de sesión ([motor §6.4](../contracts/nu9ve-engine-v1.md#64-leases-de-sesión), propuesta). Pasa los 20 casos contra su motor de prueba y contra `clipper-core serve`. Lo adopta neuf cuando Clipper cierre su migración; nüün y nove conservan los suyos |

### Ola 3 · con su condición

| Pieza | Condición |
| --- | --- |
| `data/sound/` y el motor de sonido de Swift (`FractosSoundV2`) | Cerrar la pregunta de niveles del [contrato de sonido](../contracts/nu9ve-sound-v1.md) en la Mac con el repo de la web; después, cuando una segunda app de Mac lo adopte |
| `nu9ve-ipc`, `nu9ve-mcp` | Que nüün porte su MCP a Rust: entonces hay dos copias (nove `core/src/pipe.rs`, `mcp.rs`). **Adelantada el 2026-11-19** por decisión del usuario: `nu9ve mcp` ([diseño](suite-mcp.md)) es el segundo usuario, al lado de nove, con el [registro de motores](../contracts/nu9ve-registry-v1.md) y el [catálogo de datos](../contracts/nu9ve-data-v1.md) |
| `ShortcutCatalog` y ayuda contextual (SoundBender) | Una segunda app que implemente el [contrato de interacción](../contracts/nu9ve-interaction-v1.md) |
| Controles comunes: `Nu9veFormat` y `Nu9veControls` en `swift/`, los mismos en `Nu9ve.Core`/`Nu9ve.WinUI` (valores y unidades, estados, medidor, perilla, regla de tiempo, tabla, fechas, texto) | Que el usuario responda las preguntas de [controles comunes §9](suite-ui.md#9-preguntas-para-el-usuario). Después, en el orden C1 a C14 de su [§7](suite-ui.md#7-orden-de-extracción), con la app de origen y el segundo usuario de cada pieza. El catálogo de atajos de la fila de arriba es su C6 |
| Paleta y dither en TypeScript (curado, web de Fractos) | Decidir si `nu9ve/core` tendrá una carpeta de TypeScript; hoy no |
| GPU: `Nu9veMetal` en `swift/`, kernels WGSL comunes y evaluador de `motion: declared` ([propuesta](platforms/gpu.md)) | Que el usuario responda las preguntas de la [§8](platforms/gpu.md#8-preguntas-para-el-usuario). Después, en el orden de la [§7](platforms/gpu.md#7-orden-propuesto) |
| `nu9ve-jobs`: supervisar un proceso (grupo propio, NDJSON, cancelar, timeout, publicar) | Que un segundo motor de Rust lo necesite: curado (`stems.rs`) o nüün. Antes, separar en Clipper lo genérico de su cola ([inventario](#clipper)) |
| `nu9ve-algo`: grafos (Dijkstra, A*), rejilla espacial, kNN, punto en polígono y la ruta de inspección por baches | Nació genérico en nove (`crates/nu9ve-algo`, `1555bb7`, sin dependencias) y se muda cuando tenga un segundo usuario (su A49). Candidatos: Atlas, cuando porte su primera etapa a Rust (S9 de nove), o kyu |
| `nu9ve_engine::respond` pública: atender un mensaje en el mismo proceso, sin socket | La pide SoundBender para el paso 2c de su API (renders puros por `soundbender-ffi`); mientras, usa `serve_lines` sobre un `Cursor`. Falta la propuesta en el core |

<a id="3-cómo-empieza-cada-app"></a>
## Cada app

Cada app sigue lo suyo en **sesiones de su propio repo**: adoptar una pieza, sincronizar su copia, arreglar su gate. La sesión del core hace lo común: contratos, piezas compartidas, este plan y la adopción ([cómo se trabaja](../README.md#cómo-se-trabaja)). La última columna dice dónde retoma una sesión de esa app. **Antes del siguiente paso, cada sesión revisa su fila en [CI sólo a mano](#ci-sólo-a-mano).**

<a id="ci-sólo-a-mano"></a>
### CI sólo a mano

Decisión del usuario, 2026-11-16 ([AGENTS](../AGENTS.md#sesiones-que-trabajan-en-la-suite)): los workflows de GitHub ya no corren en cada push, porque los runners de macOS (10 minutos por minuto) y Windows (2) agotaban la cuota y GitHub empezó a cobrar. La sesión de cada repo que falta lo hace en un commit propio, lo sube y marca aquí su fila:

1. En cada archivo de `.github/workflows/`, el bloque `on:` queda sólo en `workflow_dispatch`, con esta línea encima: `# Sólo a mano (Actions › Run workflow): un push no gasta minutos de GitHub. Decisión del usuario, 2026-11-16.`
   ```yaml
   on:
     workflow_dispatch:
   ```
2. Los gates locales (`validate.sh`, `sync-core.sh --check`, pruebas) no cambian: se corren antes de cada commit.
3. Si la documentación del repo dice que el CI corre en cada push, se corrige en el mismo commit.

| Repo | Estado | Commit |
| --- | --- | --- |
| core | Hecho | `f8c815f` |
| nüün (`n-creator-utils`) | Hecho | `c9bbce6` |
| nove | Hecho | `896bc5d` |
| Clipper | Hecho | `41438135` |
| curado | Falta: su `check.yml` corre en cada push, en tres sistemas, y todavía no está en Git (archivo sin rastrear en la Mac). Al subirlo, que sea ya a mano | — |
| kyu | Sin workflows | — |
| SoundBender (`soundshop`) | Hecho | `56838bb` |
| Fractos (`ios-fractos`) | Sin workflows | — |
| neuf, Stalker | Sin workflows | — |

Si el usuario quiere el corte también fuera de los repos: *Settings › Actions › General* en cada repo, o un tope de gasto de Actions en 0 USD en *Settings › Billing*. Eso lo hace él; ninguna sesión toca la configuración de GitHub.

| App | Copia fijada | Siguiente paso de la suite en su repo | Qué aporta a la suite | Dónde retoma |
| --- | --- | --- | --- | --- |
| **nüün** (`n-creator-utils`) | `contracts`, `data`, `docs`, `rust/nu9ve-json`, `rust/nu9ve-fs`, `rust/nu9ve-engine`, `swift`, `windows`; `--check` en su gate | **Perfiles (2026-11-25):** leer `…/nu9ve/profiles.json` ([`nu9ve-profile-v1`](../contracts/nu9ve-profile-v1.md)) con `scripts/nu9ve_profiles.py` o `rust/nu9ve-profiles` para saber qué perfiles hay en la máquina y en qué apps, y agregar su entrada `nuun` a `e64` (y a los perfiles que tenga) con `upsert_app_entry`; nove lo escribe primero, con `e64`. **Primero, F1 (2026-11-22):** volver a fijar el core; traer el binario `nu9ve` y ofrecer en Ajustes «Instalar el agente» / «Actualizar» con `nu9ve agent state` e `install --by nuun` ([agente §5](../contracts/nu9ve-agent-v1.md#5-cómo-lo-ve-una-app)); dejar de crear `device.json` cuando el agente esté; servir `data_catalog` y `data_export` ([`nu9ve-export-v1`](../contracts/nu9ve-export-v1.md)) por un transporte registrable; los respaldos de colecciones por dispositivo (`<colección>/<device_id>/…`); «Trabajar aquí» con `lease_take`. Probar F0 en la M4. **Después:** `NuMacShell` en lugar de su `ShellWindow`, `SkinChrome` y `AboutView`, después de la semana de uso diario. En Windows, su fase A | `nu9ve-json`, `nu9ve-volumes`, `nu9ve-time`, `Nu9ve.Core`/`Nu9ve.WinUI` | [STATUS](../../../nu9ve/n-creator-utils/docs/STATUS.md) y su PLAN |
| **Fractos** (`ios-fractos`) | `swift` y los contratos de Mac, tema y sonido | Arreglar las pruebas de su contrato de la web (`sync-contracts.sh` con `nu9ve.xyz`). Más adelante, que su motor de sonido se mude al core | `NuMacShell`, el motor de sonido | [MONOREPO](../../../nu9ve/ios-fractos/docs/MONOREPO.md) y [SUITE-DESKTOP](../../../nu9ve/ios-fractos/docs/SUITE-DESKTOP.md) |
| **Clipper** | Contratos de JSON, MCP, archivos, motor y artefactos; vectores y ejemplos; `rust/nu9ve-json`, `nu9ve-fs`, `nu9ve-engine`, `nu9ve-artifact`; este plan; `--check` en su gate (`9ec01d5`) | ~~Separar lo genérico de su supervisor~~ (`jobkit`, `0e69ab68`) y ~~validar `contract`~~ (`39c6db70`). Sigue: cerrar su migración a Rust. Al 22 nov (`ac8ecbbd`, core fijado en `c1c8444`): integradas M0–M8 y las sesiones A, A2, B, C, C2 y F del cierre. El motor ejecuta el export con un plan por proyecto que lanza un host Python por bloque, mide y decide el mastering con `nu9ve-loudness` (también la canción de la intro), y lleva el Sync, las sesiones Stalker, templates, tema, escenas de teclas, el sonido de las escenas, el WAV de análisis y la validación de handoffs consultando a nüün (`nu9ve media` como worker); desde F2 (`e29d7dd8`), también todas las escrituras de M7. Retiros E1 y E2 integrados (`0a902e65`): sin motor, Clipper falla cerrado. Sesión G integrada (`e27534e7`): huecos en el motor, retiro de proyectos y dorado sin referencias Python vivas. Sesión H integrada (`7590d6cb`): los 14 fallos resueltos y Python ya no decide sync, fase 5 ni escrituras. Falta M9 (gate, suite y benchmark completos). M8 ya da a Neuf las vistas como métodos del motor y `Nu9veEngine` el cliente Swift | Servidor de líneas, supervisor de procesos (`jobkit`), semántica de Python, sombra entre motores | [Plan de cierre](../../clipper/docs/2026-11-21-rust-migration-closure.md) y [roadmap del motor](../../clipper/docs/2026-11-13-clipper-core-roadmap.md) |
| **curado** | Mac, Windows, tema e interacción, vectores de tema, `nu9ve-fs` y su dependencia `nu9ve-json`. Fijado en `2d45333`, con `bun run core:check` y `--strict` verificados | Tras cerrar la escucha y reconexión del DDJ-SB en Mac, adoptar el medidor de `nu9ve-loudness` con sus vectores y medir el efecto en auto gain. La comparación con SoundBender ya está hecha. El limitador en tiempo real queda para un trabajo de DSP aparte. En paralelo, paridad de roles de tema en TypeScript y localización de la frontera nativa | Capas de audio y supervisión de stems. Evaluar lectura de identidad de volumen cuando exista la implementación compartida | [STATUS](../../curado/docs/STATUS.md) y [Suite](../../curado/docs/SUITE.md) |
| **nove** | **2026-11-25, A98 (en integración):** primer escritor de [`nu9ve-profile-v1`](../contracts/nu9ve-profile-v1.md): fija `nu9ve-profiles`; su motor arranca con `--profile`, lo dice en el registro y lee los demás perfiles en sólo lectura. **2026-11-24, A95 (en integración):** fija la versión menor de `nu9ve-bench` y de `nu9ve-artifact` para S10 y Salidas. Contratos de Mac, Windows, tema, interacción, MCP, avisos, motor, registro, catálogo y artefactos; vectores, ejemplos y validadores; `rust/nu9ve-json`, `nu9ve-fs`, `nu9ve-ipc` y `nu9ve-artifact`; `swift` y `windows` (`39c1b9b`) | **Primero (2026-11-22):** volver a fijar el core; F0: raíces por volumen y «disco no conectado» ([volumen](../contracts/nu9ve-volume-v1.md)); F1: «otras máquinas» con los métodos del motor `suite` (`devices`, `where`, `leases`) y `data_export` de sus datasets ([`nu9ve-export-v1`](../contracts/nu9ve-export-v1.md)); el proyecto `conversaciones` que junta las de todas las máquinas. **Después:** ~~El tema en la app de Mac~~ (`65a9e50`, su A46). Hecho el 2026-11-20: se registra en la suite, sirve `data_catalog` (también series y conversaciones de agentes, `personal`) y `mcp_tools`/`mcp_call`, y exporta artefactos. **Sigue:** recorrer con el usuario, en la app instalada, Atlas, Conversaciones, Explorador y Resultados; probar `nu9ve mcp` con un cliente real; en Windows, ponerse al día (va en datos 27 frente a 33) y revisar a mano la ventana del broker y el alto contraste | IPC enmarcado con ACL y MCP con niveles; primer motor en el registro; primer productor de artefactos; `nu9ve-algo` | [STATUS](../../nove/docs/STATUS.md) y [Seguir en Windows](../../nove/docs/windows-next.md) |
| **SoundBender** (`soundshop`) | Contratos de Mac, tema, interacción y sonido, vectores de tema y `swift`; `--check` en `validate.sh` | Ventana: pasos 1 a 3 hechos (`1cf31e3`, `29b0e90` y `8a79f82`) y barra `PeriodWindowBar` de `NuMacShell` (`2ab211d`), sin probar a mano. Core fijado en `bcd8d43` con su LUFS medido contra Swift (`41ab675`, `16ac1c8`). Escena espacial 3D en Metal con su cálculo en Rust por `soundbender-ffi` (`62617a5`, `298a85f`) y presupuesto de memoria de la mitad de la RAM (`4045b8c`, su decisión 040), sin publicar. Motor portable en Rust (`engine/`): 14 de 23 dorados con los mismos bits o dentro de la tolerancia; usa `nu9ve-loudness` (`3df4dfb`). API de comandos de su decisión 037: pasos 2a y 2b hechos (`7f1e85a`, `dac529d` a `90e65d1`), 2b sin probar en la app. Desde el 2026-11-19, sin publicar: secciones y asas (su decisión 041), racks y pincel de patrón (042) y una escena que se lee como suena, con altura por tono, formas por sonido y análisis v3 en el motor (043, `19ef65c`). **Sigue:** pruebas a mano y publicar la v0.32; proponer aquí una `nu9ve_engine::respond` pública para el modo en proceso del paso 2c (renders puros), que ya empezó con la escena; después, medidas y tipografía de cada skin | `ShortcutCatalog`; segundo usuario de loudness; candidato a G1 y G7 de la [propuesta de GPU](platforms/gpu.md) | [Ventana y skins de la suite](../../soundshop/docs/plans/suite-shell.md) |
| **kyu** | Contratos de Mac, tema e interacción, vectores de tema y `swift`; `--check` en `validate.sh` | ~~`NuMacShell` en lugar de su ventana y cromo~~ (`77a3449`). Sigue: sumar su columna en [adopción de Mac](adoption.md#macos-app-v1) | Segunda verificación de los vectores en Swift; la regla de `slug()` | [STATUS](../../kyu/docs/STATUS.md) |
| **neuf** | Contratos de Mac y tema, vectores de tema y `swift` (`a70b717`) | ~~Primera copia fijada~~. Sigue: `NuMacShell` en lugar de su `MacAppShell`. Sus flujos esperan a que Clipper cierre su migración a Rust (decisión del usuario, 2026-11-21); después, el cliente Swift de `nu9ve-engine-v1` | — | [STATUS](../../neuf/docs/STATUS.md) |
| **Stalker** | Contratos de Mac, interacción, avisos y archivos y `swift` (`2f67cc8`); `--check` en `scripts/check.sh`. Usa `AboutView` y `AppIdentity` | ~~Menú, Acerca de, `Info.plist` y modo QA~~ (`d5c48d7`). Sigue: la señal de grabación como contrato y la ruta de notarización | La señal de grabación (`sessionID` y epoch, propuesta en [navegador §7](suite-browser.md#7-contratos-nuevos)) y el reloj de sesión | [Stalker en la suite](../../stalker/docs/2026-11-18-suite-y-prioridades.md) |

El primer paso de una app sin copia no mueve código: declarar en `vendor/nu9ve-core.paths` lo que ya sigue, fijarlo, poner `--check` en su gate y apuntar sus enlaces a la copia.

<a id="sesiones-de-unificación"></a>
## Sesiones de unificación

Propuestas el 2026-11-22, tras revisar todos los repos el mismo día. Son sesiones **aquí, en el core**, que dejan una pieza lista para que las apps la tomen en sus propias sesiones. El orden es el recomendado; cada una es un cambio propio, con su prueba y su línea en el CHANGELOG.

| # | Sesión | Qué deja | Quién la toma después | Condición |
| --- | --- | --- | --- | --- |
| U1 | **Hecho el 2026-11-22.** `sync-core.sh` para dos repos ([apps con y sin la parte privada §5](suite-modules.md#5-cómo-se-fija-inner-core)) | El script reconoce `nu9ve/core` y `nu9ve/inner-core` por `.nu9ve-repo`, con `--repo`, sin cambiar nada para quien no lo pasa; pruebas nuevas en `test-sync-core.sh` | Toda app que fije `inner-core` y el propio `inner-core`, que fija el core | Ninguna |
| U2 | **Hecho el 2026-11-22.** Mudar `nu9ve-volumes` con `nu9ve-volume-v1` | El crate en `rust/`, con el marcador, los estados y el sobre del índice ([rust/README](../rust/README.md#volúmenes)); validar el índice como catálogo sigue en la app | nüün primero (la hace su sesión, que ya tiene el código); después nove, Atlas y Clipper para leer marcadores | El F0 de nüün, ya en `c85b881` |
| U3 | **Hecho el 2026-11-22**, salvo correr C# en Windows. Color en los skins de época ([tema](../contracts/nu9ve-theme-v1.md)) | Una opción «Época: Original o Con tu color» para Macintosh y Windows 9x, que acepta tu color y tu modo sin cambiar las medidas; por omisión, como hoy. En `Nu9veTheme`, `Nu9ve.Core` y vectores nuevos | Todas las apps con esos skins, al volver a fijar | Que el usuario apruebe la propuesta |
| U4 | **Hecho el 2026-11-22** (falta que video y Clipper lo adopten). `Nu9veVision` en `swift/` | Detección con Apple Vision (caras, cabezas, personas, máscara de segmentación), con caché rápida y precisa y un formato de resultado común | video y Clipper | Leer bien las dos copias ([mapa](suite.md#lo-que-ya-se-repite-y-podría-compartirse)) y que sus sesiones acepten |
| U5 | **Hecho el 2026-11-22** (con revisiones, decisión del usuario; falta que Clipper y video lo adopten). `Nu9veScore` en `swift/` | El compositor procedural, desde `ClipperScore`, con lo que sumó la copia de video | Clipper (app de origen) y video | Que la sesión de Clipper lo acepte mientras cierra su migración |
| U6 | **Hecho el 2026-11-22**, desde curado tras comparar (falta que curado y SoundBender lo adopten). `nu9ve-music` en `rust/` | BPM, grid y clave, con vectores ([audio](platforms/audio.md)) | curado y SoundBender | Comparar antes las dos implementaciones, como se hizo con `nu9ve-loudness` |
| U7 | **Hecho el 2026-11-22**, en QA. U7a: la decisión de quién instala el agente (pregunta 9), [`nu9ve-agent-v1`](../contracts/nu9ve-agent-v1.md), [`nu9ve-exchange-v1`](../contracts/nu9ve-exchange-v1.md) y la regla nueva de lo personal en `nu9ve-data`. U7b: `nu9ve-device`, `nu9ve-exchange` y `nu9ve agent` (init, publicar, leer, emparejar). **U7c:** `serve` (un agente por máquina, kqueue, cierre ordenado), `install`/`uninstall`/`state` con el LaunchAgent, el motor `suite` con `suite_devices`, `suite_volumes`, `suite_where`, `suite_leases`, `suite_lease_take` y `suite_pull`, y pedidos, objetos, dueñas y `apps/<app>/` ([CHANGELOG](../CHANGELOG.md)). **Instalado en la M1** con su LaunchAgent (sin carpeta de intercambio todavía). **Sin verificar:** la M4, Windows y Syncthing entre las dos. **Queda para después:** el canal propio entre agentes (decisión del usuario, pregunta 2) | `nu9ve-exchange-v1`, el agente `nu9ve` (identidad, emparejado, carpeta, avistamientos, pedidos) y su canal propio entre máquinas | nüün (lo trae y lo ofrece), nove («otras máquinas») | Probar en la M1 y la M4 con Syncthing, y `nu9ve mcp` con un cliente real |
| U8 | **Hecho el 2026-11-22.** Guía de prácticas ([`docs/practices.md`](practices.md), Ola 2) | Lo que ya repiten cinco apps: QA aislada, instalación con respaldo, gate con `--check`, CHANGELOG | Todas | Ninguna |

**En las apps, a la par:** la prioridad es **nove como base de los flujos de Atlas** (misiones, tandas, baches, tramos, tráfico), para que en el corto plazo nove haga todo eso nativo y en red con nüün. Atlas sólo hace lo urgente de F0 y queda como laboratorio (decisión del usuario, 2026-11-22; ya estaba en nove A49).

### Copias fijadas al 2026-11-22

Releído en cada repo al cerrar U7c, con el core en `34e091f` (sin `git fetch`: lo que dice cada checkout de la M1). «Detrás» es cuántos commits del core hay desde el fijado. Volver a fijar es un cambio propio en cada app, cuando le convenga. Desde U7c cambió `nu9ve-engine` (sólo suma: `RpcError` con `Display`), así que el `--check` de nüün, Clipper y SoundBender avisa.

| App | Core fijado | Detrás | Estado del repo |
| --- | --- | --- | --- |
| nüün (`n-creator-utils`) | `9711564` (U2) | 16 | Limpio y al día con su remoto. Último: `01b4c20`, el índice del disco por proyecto y carpeta |
| Clipper | `c1c8444` | 27 | Al día con su remoto; sólo `.claude/` sin rastrear. Último: `d0c9d6f9` (ola R7 integrada; faltan el ejecutor del export en Rust, R7e, y las decisiones restantes del export, R7f) |
| curado | `1114e02` | 22 | 66 cambios sin commit (54 modificados, 3 borrados, 9 sin rastrear): sigue mudando su lectura MIDI a `nu9ve-midi`. Último commit: `bb6f780`, del 2026-11-17 |
| nove | `39c1b9b` | 39 | Limpio. Último: `a03a8e2` (modo proyecto, A69). Tres worktrees de `.claude/` fijan `25337e6` |
| SoundBender (`soundshop`) | `bcd8d43` | 48 | Limpio, con un commit sin subir (`add66ed`, la medida de la escena) |
| kyu | `9cd8a82` | 56 | Limpio |
| Fractos (`ios-fractos`) | `9cd8a82` | 56 | Limpio |
| Stalker | `2f67cc8` | 47 | Limpio |
| neuf | `a70b717` | 51 | `.gitattributes` sin rastrear; en pausa hasta que Clipper cierre |
| Atlas, video | Sin copia | — | Limpios. Atlas, puesto al día el 2026-11-23 en `c0adf5f` (iteraciones 52 a 60: capas, cobertura de la ortofoto y `atlas-needs-v2` con un `serve --share` que la suite no usa: Atlas es laboratorio y nove lo importa con utilidades desechables, [pregunta 24](suite-devices.md#12-preguntas-para-el-usuario)); video: `cf3f132` |
| Web de Fractos (`nu9ve.xyz`) | Sin copia | — | En `develop`, tres commits sin subir y 20 archivos sin commit; último commit del 2026-10-27, sin rastro del OAuth en esta Mac |

## Inventario por app

Lo que cada app repite de la suite o de otra app. Los paralelos no implican la misma API ni una extracción aprobada. El código sigue en su repo hasta que su pieza entre en una ola.

<a id="inventario-de-solapamientos-en-nüün"></a>
### nüün

Revisado el 2026-11-16. Los tamaños son líneas del archivo de nüün, salvo los intervalos indicados. Los paralelos no implican la misma API ni una extracción aprobada.

| Pieza | Archivo en nüün y tamaño | Quién más la tiene |
| --- | --- | --- |
| Roles, contraste, presets y preferencias | `MediaMenuCore/ThemeColor.swift` pasó a `swift/Sources/Nu9veTheme`; `MediaMenu/Appearance.swift` conserva el estado y las vistas. El port de C# pasó a `windows/Nu9ve.Core`; nüün declara su identidad en `Nuun.Core/Theme/NuunTheme.cs`. | Kyū comparte el cálculo de los roles y conserva sus superficies y colores de dominio. SoundBender: `Sources/SoundShop/Appearance.swift`; curado: `src/lib/theme.ts`. |
| Ventana, agarres de borde y controles de skin | `native/MediaMenu/Sources/MediaMenu/ShellWindow.swift` (243), `native/MediaMenu/Sources/MediaMenu/SkinChrome.swift` (411) | core: `swift/Sources/NuMacShell/{ShellWindow,WindowChrome}.swift`, ya consumidos por Fractos; kyu conserva `apps/macos/Sources/Kyu/SkinChrome.swift`. La ventana se migra después de la semana de uso diario. |
| Acerca de e identidad de build | `native/MediaMenu/Sources/MediaMenu/AboutView.swift` (50), `native/MediaMenu/Sources/MediaMenuCore/AppIdentity.swift` (74) | core: `swift/Sources/NuMacShell/{AboutView,AppIdentity}.swift`, usados por Fractos; SoundBender tiene su propio `Sources/SoundShop/AboutWindow.swift`. |
| Catálogo de atajos | `windows/Nu9ve.Core/Shell/Shortcuts.cs` (54, ya en el core) y `native/windows/Nuun.Core/Shell/ShortcutCatalog.cs` (36) | SoundBender: `Sources/SoundShop/AppCommands.swift` y `ShortcutsPanel.swift`. Se puede comparar la validación de acciones; cada catálogo conserva los atajos de su app. |
| JSON canónico, digest e id aleatorio | `media/common.py:52–61` (10) | core: `rust/nu9ve-json`; Clipper conserva su serialización Python en `core/src/rpc_jobs/pyval.rs`. La copia Python de nüün sigue como oráculo; el motor Rust ya usa el crate fijado. |
| Escritura atómica y sincronización de archivos | `engine/crates/nuun-media/src/tree.rs:251–286` (36), `engine/crates/nu9ve-volumes/src/fs.rs` (626; incluye otras operaciones) | Clipper: `core/src/rpc_writes/fsio.rs`. Las primitivas comunes viven en `nu9ve-fs`. El motor conserva las comprobaciones de rutas y usa `Full`. |
| Supervisión de subprocesos | `engine/crates/nuun-media/src/integrations.rs:390–480` (91), `native/MediaMenu/Sources/MediaMenuCore/Protocol.swift` (378; incluye el protocolo) | Clipper: `core/src/rpc_jobs/supervise.rs` y submódulos. Se repiten el manejo de tuberías, salida y terminación; el lector persistente y los trabajos de Clipper tienen ciclos de vida distintos. |
| Separación de stems | `manager/file.py:885–889` (5, `split_file`) | curado: `src-tauri/native/separate.py`; SoundBender: `Sources/SoundShop/Resources/separate.py`. Lo común es lanzar Demucs y manejar su resultado; nüün no implementa el DSP. |

<a id="4-clipper-qué-de-su-rust-sirve-a-la-suite"></a><a id="inventario-de-solapamientos-en-clipper"></a>
### Clipper

Leído el 2026-11-16 en `clipper` (`main`, `0dd5d81`). `clipper-core` es un solo crate de unas 39 000 líneas, `edition = "2024"`, `rust-version = "1.89"`, cuya única dependencia es `serde_json` (con `arbitrary_precision` y `preserve_order`). Igual que nüün, porta a Rust un motor de Python método por método, con la respuesta byte a byte como criterio y `not_ported` para lo que falta. Ese camino común es la razón de casi todo el solapamiento.

| Pieza de Clipper | Qué hace | Ya existe en la suite | Qué haría falta |
| --- | --- | --- | --- |
| Sobre de `main.rs` (`{id, ok, data}` / `{id, ok: false, error: {code, message}}`) | Respuestas del protocolo `clipper-engine/1` | Es el mismo sobre de [`nu9ve-json-v1` §9](../contracts/nu9ve-json-v1.md) (`ok_envelope`, `error_envelope`) | **Hecho el 2026-11-16** (`6312f727`), sin cambios en el protocolo |
| `py::sha256_hex` (SHA-256 escrito a mano), `fsio::random_bytes`, `fsio::uuid4` | Digest de esquemas y reglas, ids | `nu9ve-json`: `digest`, `hex`, `random_bytes`, `uid` (sobre `sha2`) | **Hecho el 2026-11-16** (`6312f727`): `sha256_hex` y `uuid4` salen del crate; la copia a mano se borró |
| `py::float_repr`, `pyval::dumps` (los formatos de `json.dumps`, con indentación incluida), `round_to`, `casefold`, `strip`; `paths.rs` (`pathlib` de POSIX) | Reproducir CPython para que las respuestas coincidan con Python | `nu9ve-json::python_float_repr` y el JSON canónico; `nu9ve-time` en nüün para `datetime` | **Candidato nuevo, `nu9ve-py`**: «semántica de Python» para los dos motores que portan desde Python. El JSON canónico se queda en `nu9ve-json`. Lo de `json.dumps(indent=2)`, `round`, cadenas y `pathlib` va al crate nuevo, con vectores generados por CPython como `data/json-vectors-v1.json` |
| `rpc_writes/fsio.rs` (`atomic_write` con `mkstemp`, fsync y rename, más fsync del directorio; `locked_path` con `flock`) y otra `atomic_write` en `rpc_jobs/store.rs` | Escribir y bloquear igual que `storage.py` | `nu9ve-volumes/src/fs.rs` en nüün: `full_sync`, `sync_dir`, `create_new`, leer sin caché | **Candidato nuevo, `nu9ve-fs`**: separar de `nu9ve-volumes` la parte de archivos (lo que no es identidad de disco) y sumar escritura atómica y bloqueo. Clipper sería el segundo usuario y curado el tercero. Ojo: Clipper usa `fsync(2)` simple a propósito, para igualar a Python, mientras `File::sync_all` en macOS hace `F_FULLFSYNC`. Hay que decidirlo en el contrato |
| `rpc_jobs/supervise.rs` (≈2 500 líneas con `publication` y `sequence`) | Supervisa un proceso: grupo propio, eventos NDJSON, cancelar por señal, archivo o cola, timeout y bloqueo con SIGTERM y luego SIGKILL, y publicación atómica de lo que produjo | Nada todavía. Lo mismo, en otra forma, en los stems de curado (Demucs aislado), en `separate.py` de SoundBender y en las transferencias de nüün | **Candidato de la ola 3, `nu9ve-jobs`**: el supervisor genérico sin el manifiesto `clipper-job/1`. Entra cuando un segundo motor de Rust (curado o nüün) lo necesite. Antes hay que separar en Clipper lo genérico de lo que conoce su cola |
| `rpc_jobs/progress.rs` (progreso de FFmpeg `-progress`, archivo `fracción\tetiqueta`, ETA) | Progreso de los workers | — | Se queda en Clipper por ahora. Es candidato a acompañar a `nu9ve-jobs` si nüün o curado leen el progreso de FFmpeg |
| `ClipperScore` (DSP en Swift, según `docs/2026-11-12-native-core-migration.md`) | Análisis de audio | Las capas de audio en propuesta (`nu9ve-loudness`, `nu9ve-dsp`) | Si Clipper lo porta a Rust, que use o alimente `nu9ve-loudness` y no un tercer limitador o LUFS |
| Timeline, cortes, subtítulos, framing, multicam, overlays, miniaturas, `program_clock`, cola de exportación | El dominio editorial | — | Se queda en Clipper |

**Para Clipper, después de `nu9ve-json`:** `nu9ve-fs` primero, porque ya tiene dos `atomic_write` que no se comportan igual. `nu9ve-py` se diseña con nüün. Su sobre ya es el de `nu9ve-json`, así que `nu9ve-engine-v1` describe lo que Clipper ya habla.

Revisado el 2026-11-16 sobre `4d3ad12`. Los tamaños son líneas del archivo de Clipper, salvo los intervalos indicados. Los paralelos no implican la misma API ni una extracción aprobada.

| Pieza | Archivo en Clipper y tamaño | Quién más la tiene |
| --- | --- | --- |
| Ventana, menús y Acerca de | `apps/ClipperStudio/ClipperStudio/ClipperStudioApp.swift` (38): `WindowGroup`, `Settings` y un `CommandMenu`, con el Acerca de del sistema | core: `swift/Sources/NuMacShell`, ya en Fractos; nüün `ShellWindow.swift` y `AboutView.swift`; SoundBender `AboutWindow.swift`. ClipperStudio no tiene cromo propio que borrar: adoptar `NuMacShell` es sumar, no reemplazar |
| Atajos y ayuda contextual | TUI: `src/clipper_tui/command_help.py` (500; paleta, hoja `?` y teclas fuera de los bindings) y `SHORTCUT_SPECS` en `app.py`; Mac: tres `keyboardShortcut` en `ClipperStudioApp.swift` | SoundBender `ShortcutsPanel.swift` y `AppCommands.swift`; curado `src/lib/keyboard.ts`; nüün `Shortcuts.cs`. El de Clipper es un catálogo único para paleta, ayuda y pruebas, como pide el [contrato de interacción](../contracts/nu9ve-interaction-v1.md), pero en Textual |
| Tema | TUI: `CSS_TEMPLATE` en `src/clipper_tui/app.py`; `theme_kits.py`, `theme_components.py`, `motion_theme.py` y `ThumbnailArtDirection.swift` (1 984) son estilos del video, no de la interfaz | Ningún paralelo directo: los kits de tema de Clipper son del dominio editorial. `Nu9veTheme` sólo tocaría a ClipperStudio si adopta skins |
| JSON canónico y digest | Rust: `core/src/rpc_jobs/pyval.rs` y `core/src/py.rs` (formatos de `json.dumps`); Python: 41 llamadas `sha256`/`sha1(json.dumps(...))` en 33 módulos de `src/clipper_tui/` con separadores, `ensure_ascii` y `default=str` distintos; sólo 7 usan la forma compacta con `ensure_ascii=False` | core: `rust/nu9ve-json` (Clipper ya toma de ahí el sobre, `sha256_hex` y `uuid4`); nüün `media/common.py:52–61`. Esos digests son claves de caché y firmas internas: cambiar su forma invalida cachés, así que no conviene unificarlos sin un motivo |
| Escritura atómica y bloqueo | Python: `src/clipper_tui/storage.py:63–123` (61, `locked_path`, `atomic_write_text`, `atomic_write_json`), `media_access.py` (75) y otros `fcntl.flock`/`os.replace` sueltos (`export_queue.py`, `engine_jobs_queue.py`, `creative_profiles.py`, `media_handoff.py`, `overlay_scenes.py`, `motion_render.py`); Rust: [la tabla de arriba](#nu9ve-fs-en-clipper-qué-es-genérico-en-fsiors-y-storers) | nüün `nu9ve-volumes/src/fs.rs` (626) y `nuun-media/src/tree.rs:251–286`; curado `rekordbox_xml.rs` (`write_atomic`, 11); SoundBender `ProjectStorage.swift` (`.atomic`) |
| Supervisión de procesos | Rust: `core/src/rpc_jobs/supervise.rs` (1 258), `supervise/publication.rs` (724), `supervise/sequence.rs` (525), `rpc_jobs/progress.rs` (472); Python: `src/clipper_tui/processes.py` (185; grupo propio, `killpg`, SIGTERM y luego SIGKILL, apagado de la app) | curado `src-tauri/src/stems.rs` (1 284) y `tracking.rs` (439); nüün `nuun-media/src/integrations.rs:390–480`; SoundBender `separate.py`. Candidato `nu9ve-jobs` de la ola 3 |
| Audio: loudness y limitador | `src/clipper_tui/master_audio.py` (400): −14 LUFS y −1 dBTP con FFmpeg (mide con `ebur128`, limita con `alimiter`), con caché de medidas; `audio_features.py` decodifica con FFmpeg; `core/src/rpc_score.rs` (705) | curado `audio-engine/src/loudness.rs` (92) y `limiter.rs` (266); SoundBender `MasterLimiter.swift`. Para `nu9ve-loudness`, Clipper es consumidor de las medidas, no implementación: mide con FFmpeg |
| Audio: síntesis | `native/clipper-macos-render/Sources/ClipperScore/` (2 491 en siete archivos: DSP, batería, efectos, voces, motor), `AudioCrossfade.swift`, `GeneratedIntroAudio.swift` | Fractos `FractosSoundV2` (sonido de interfaz, otro propósito); SoundBender `SoundCore`. La decisión de pasar el compositor a Rust está tomada en Clipper; cuando ocurra, comparar con `nu9ve-dsp` antes de escribir un tercer DSP |

<a id="nu9ve-fs-en-clipper-qué-es-genérico-en-fsiors-y-storers"></a>
#### `nu9ve-fs` en Clipper: qué es genérico en `fsio.rs` y `store.rs`

Leído el 2026-11-16 en `clipper` (`main`, `4d3ad12`). Inventario previo a la extracción. Los tamaños son líneas de la función o del intervalo, en `core/src/`. La [adopción](adoption.md#nu9ve-fs-v1) recoge las llamadas que ya migraron.

| Función | Dónde y tamaño | Genérica o de Clipper | Qué la ata a Clipper o a Python |
| --- | --- | --- | --- |
| `fsync_file` | `rpc_writes/fsio.rs:16–32` (17) | Genérica | `fsync(2)` simple por `extern "C"`, reintentando EINTR como CPython. Sirve también para directorios. Es la alternativa a `full_sync` (`F_FULLFSYNC`) de `nu9ve-volumes/fs.rs`: el contrato debe ofrecer las dos y decir cuál usa cada app |
| `atomic_write` | `rpc_writes/fsio.rs:138–189` (52) | Genérica | Temporal hermano `.<nombre>.XXXXXXXX.tmp` con el alfabeto de `tempfile`, modo 0600, `create_new`, hasta 100 intentos; fsync, rename y fsync del directorio, ignorando sólo un directorio que no abre. Los ocho caracteres salen de `nu9ve_json::random_bytes`. Igual a `storage.atomic_write_text` de Python |
| `lock_exclusive` y `Held` | `rpc_writes/fsio.rs:34–79` (46) | Genérica | `.<nombre>.lock` al lado, abierto como `"a+"` de Python, `flock` exclusivo con sondeo cada 10 ms hasta un plazo. El plazo y el nombre del candado serían parámetros |
| `media_shared` | `rpc_writes/fsio.rs:81–126` (46) | Mitad y mitad | El `flock` compartido es genérico. El token del dueño en JSON, las variables de entorno y el `NotPorted` son el protocolo de `media_access.py` de Clipper |
| `random_bytes`, `uuid4` | `rpc_writes/fsio.rs:128–137` (10) | Ya en la suite | Envoltorios de `nu9ve-json` |
| `atomic_write` y `temporary_token` | `rpc_jobs/store.rs:262–326` (65) | Genérica, segunda copia | **No hace lo mismo que la de `fsio.rs`**: usa `File::sync_all` (`F_FULLFSYNC` en macOS), saca el nombre de un xorshift sobre el reloj, el pid y un contador, ignora un fsync del directorio que falla y acepta un nombre sin carpeta (`.`). Python sí propaga ese fallo. La usan también `supervise.rs:627` y `supervise/sequence.rs:305`. Al adoptar `nu9ve-fs`, las dos se vuelven una |
| `lock_path` y `Guard` | `rpc_jobs/store.rs:82–118` (37) | Genérica, segunda copia | El mismo bucle que `lock_exclusive`. El plazo sale de constantes de Clipper (`queue.lock_wait_seconds`, su máximo y su variable de entorno) y los errores son `queue_busy` y `queue_unavailable` |
| `probe_runner`, `runner_held`, `is_running` | `rpc_jobs/store.rs:120–165` (46) | Genérica en la forma | «¿Alguien tiene este `flock`?», con y sin crear el archivo. El nombre `runner.lock` y el `Runner::Absent` son de la cola. `queue.rs:55–66` repite `is_running` |
| `Queue`, `Snapshot`, `read`, `write`, `check_shape`, `view`, `ledger`, `record` | `rpc_jobs/store.rs` (el resto, unas 230) | De Clipper | El formato de `queue.json`, su revisión por SHA-256 y el libro de la cola |

Otras primitivas del mismo tipo, escritas en línea, que convendría pasar por `nu9ve-fs` cuando exista: `place`, `exclusive_move` y `stage_copy` en `rpc_jobs/supervise.rs:913–990` (78; publicar sin reemplazar con `hard_link`, con copia si cruza de volumen y rename si el sistema no tiene enlaces, como exFAT), que equivalen a `rename_new` de `nu9ve-volumes/fs.rs`; `Journal::save` en `supervise/publication.rs:347–366` (temporal de nombre fijo bajo el candado del plan); el candado del plan en `supervise/sequence.rs:236–251` (`flock` sin espera, modo 0600); y `media_gate` en `rpc_jobs/queue_ops.rs:585–603`, otro `flock` compartido sobre el candado de medios sin el token.

<a id="5-curado-audio-workers-y-otras-coincidencias"></a>
### curado

Inventario del 2026-11-16 sobre `754afc7`. Los tamaños cuentan líneas del archivo completo, incluidas sus pruebas, salvo donde se indica la función. El código sigue en su repo. [Audio en la suite](platforms/audio.md) describe las capas propuestas.

| Pieza | Archivos y tamaño en curado | Quién más lo tiene y qué conviene comparar |
| --- | --- | --- |
| LUFS y limitador | `crates/audio-engine/src/loudness.rs` (92), `limiter.rs` (266) | SoundBender `Sources/SoundCore/MasterLimiter.swift` mide BS.1770 y limita; Clipper `src/clipper_tui/master_audio.py` mide con FFmpeg. Para `nu9ve-loudness`, comparar umbrales, picos entre muestras y latencia con las mismas señales |
| Entrada y remuestreo | `crates/audio-engine/src/decode.rs` (680), `resample.rs` (191) | SoundBender `Sources/SoundShop/AudioIO.swift` usa AVFoundation; Clipper decodifica con FFmpeg en `src/clipper_tui/audio_features.py`. Para `nu9ve-audio-io`, separar PCM, metadatos y remuestreo de los límites de memoria, cancelación, forma de onda y construcción del clip. Las posiciones existentes conservan el retardo del codificador |
| Pitch y estiramiento | `crates/audio-engine/src/pitch.rs` (426) | SoundBender `Sources/SoundCore/SampleRecipe.swift` aplica `RegionTransform` fuera de tiempo real. En curado, separar el procesamiento Signalsmith del transporte y de las voces; comprobar asignaciones, anticipación y saltos antes de compartir |
| Análisis musical y sonda | `crates/audio-engine/src/music.rs` (337), `crates/audio-engine/examples/music_probe.rs` (49) | SoundBender `Tempo.swift` y `ChromaAnalysis.swift` en `Sources/SoundCore`; Clipper tiene análisis offline de intros. La sonda genera un acorde y pulsos de 120 BPM y mide análisis y render de dos decks con key lock. Sirve para diseñar casos de `nu9ve-music`, con tiempos de diagnóstico específicos de la máquina |
| Demucs y supervisión | `src-tauri/src/stems.rs` (1 284), `src-tauri/native/separate.py` (190); otro supervisor de cámara en `src-tauri/src/tracking.rs` (439) | Clipper `core/src/rpc_jobs/supervise.rs` y sus módulos ([§4](#4-clipper-qué-de-su-rust-sirve-a-la-suite)); SoundBender `Sources/SoundShop/Resources/separate.py`; la guía de audio registra también nüün. Para `nu9ve-jobs`, comparar spawn, eventos NDJSON, cancelación, espera y vigilancia del padre (stdin en Unix, handle de proceso en Windows). El modelo, manifiesto, cuotas y registro en SQLite pertenecen al adaptador de stems o a la app |
| Identidad y volúmenes | `src-tauri/src/identity.rs` (282), `src-tauri/src/relink.rs` (491) | nüün `nu9ve-volumes`. La búsqueda, los fingerprints y la identidad portable del volumen tienen límites distintos. Candidato de ola 2 |
| Escritura atómica | `src-tauri/src/rekordbox_xml.rs`, `write_atomic` (11) | Clipper `core/src/rpc_writes/fsio.rs`; SoundBender `Sources/SoundCore/ProjectStorage.swift` usa `.atomic`. La exportación XML usa un temporal con nombre fijo y rename; el bloqueo y la sincronización explícita son diferencias para `nu9ve-fs` |
| Tema y dither | `src/lib/theme.ts` (328), `src/lib/dither.ts` (37) | Fractos web `packages/styles` según [el mapa de la suite](suite.md#lo-que-ya-se-repite-y-podría-compartirse); kyu `KyuKit/ThemePalette.swift` y `Kyu/SkinChrome.swift`. Los vectores fijados dan una referencia para los roles de acento |
| Cromo de ventana | `src-tauri/tauri.macos.conf.json` (21), configuración de overlay y semáforo nativo | `swift/Sources/NuMacShell/WindowChrome.swift` y `ShellWindow.swift` en core; SoundBender `Sources/SoundShop/WindowChrome.swift`. En Tauri se comparte el contrato de comportamiento y la configuración equivalente |
| Atajos | `src/lib/keyboard.ts` (400), `src/lib/mixer-keys.ts` (206) | SoundBender `Sources/SoundShop/ShortcutsPanel.swift` tiene catálogo y badges. Comparar detección de conflictos, etiquetas por plataforma y ayuda contextual antes de extraer; los controles de mezcla quedan en la app |

La revisión no encontró una implementación propia de Acerca de ni un serializador JSON canónico. El manifiesto de stems usa `serde_json::to_vec_pretty`; su SHA-256 de archivos lee por bloques en `identity.rs`, mientras `nu9ve-json::sha256_hex` opera sobre bytes en memoria.

## Orden

Actualizado el 2026-11-16, con lo de arriba hecho y un PC con Windows otra vez disponible. El punto 3 y el 5, el 2026-11-20.

1. **Windows:**
   - ~~nove: C#, named pipe, instalar y retirar el sobre viejo~~: hecho el 2026-11-16 (0.12.0.54 instalada, sólo v1).
   - ~~Mudar `Nu9ve.Core` y `Nu9ve.WinUI` de nüün a `windows/`~~: hecho el 2026-11-16, y nove ya lo usa con Puerto como original.
   - nove: revisar a mano la ventana del broker y el alto contraste con el tema, e instalar una versión que lo traiga.
   - La fase A de nüün en Windows ([qué se muda §5](suite-split.md#5-c-nativewindows)): pasos 1 a 3 hechos el 2026-11-16 (`n-creator-utils` `5fa3bf8`, ingreso desde la ventana). Sigue el 4, con una SD y un disco USB reales.
   - **Lo que nove y nüün repiten en Windows** ([propuesta](windows-shared-next.md)). Primero el usuario decide dos cosas: cómo se cancela un trabajador (nüün usa un evento con nombre, que no está en la §6.2 de `nu9ve-engine-v1`) y si hay un solo certificado de desarrollo. Después, en este orden: scripts de QA, cliente de motor en C# con Job Object, avisos y barra de tareas.
   - Sin versión de PowerShell de `sync-core.sh` (decisión del usuario, 2026-11-16): en Windows se corre el de bash en Git Bash, que viene con Git.
2. **CI de GitHub sólo a mano** desde el 2026-11-16 (core, nüün, nove, Clipper y SoundBender; kyu, Fractos, neuf y Stalker no tienen workflows; curado lo deja a mano al subir el suyo): cada sesión corre las pruebas en local y el CI se lanza a mano antes de una release. Ninguna sesión lo ha visto en verde.
3. **`NuMacShell` y `Nu9veTheme` en las demás apps de Mac:**
   - nüün, después de su semana de uso diario;
   - ~~Kyū~~ (`77a3449`); ~~SoundBender~~ (pasos 1 a 3 y `PeriodWindowBar`, sin probar a mano);
   - ~~nove: el tema de la suite en su app de Mac~~ (`65a9e50`);
   - ~~primera copia fijada en neuf~~ (`a70b717`) y en Stalker (`9777275`); sigue `NuMacShell` en neuf;
   - volver a fijar nüün, Fractos y Kyū, que siguen en `9cd8a82`, sin `PeriodWindowBar`.
4. Guía de prácticas comunes (`docs/practices.md`).
5. Lo que espera su condición:
   - `nu9ve-jobs`, desde Clipper y curado;
   - ~~`nu9ve-loudness`~~: hecho el 2026-11-17 y usado por SoundBender; falta que curado lo adopte;
   - `nu9ve_engine::respond`, para SoundBender;
   - `nu9ve-algo`, desde nove;
   - `nu9ve-volumes`, cuando se decida la identidad portable;
   - `nu9ve-py`;
   - el sonido de Fractos.
6. ~~Comparar `scripts/check-boundary.sh` con `check_suite_boundary.sh` de nüün y dejar uno~~: hecho el 2026-11-16, queda el del core; nüün lo corre desde su copia sobre sus `nu9ve-*`.
