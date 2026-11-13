# Plan de trabajo

Actualizado: 2026-11-12, America/Hermosillo. Éste es el backlog priorizado. [STATUS](docs/STATUS.md) dice qué funciona hoy y con qué evidencia; [CONTEXT](docs/CONTEXT.md) guarda las decisiones. La historia por entrega está en [releases/](docs/releases/status-log-2026-10-25_2026-11-08.md) y no es otra lista de pendientes.

## Cerrado (no reabrir sin un hecho nuevo)

- **Sony:** retiro de clips verificados, índices conservados, cierre automático al reconectar ya formateada. Baches `060b585d` cerrado por observación el 8 nov. Lo único abierto es mirar, en el próximo ingreso normal, que el retiro sobreviva a remontar sin formatear.
- **Dron, ingreso completo:** una SD del Mavic se respalda, se verifica, se copia el trabajo sin duplicados y queda vacía, siempre que el export de subaereo ya esté incorporado. Si falta, A14 bloquea la limpieza a propósito.
- **Cierre de Terra:** aplicado el 8 nov.

## P0 · uso diario sin sorpresas

| Orden | Trabajo | Criterio de cierre |
| --- | --- | --- |
| 1 | Aceptar en la app la build `a350af7f16af` | Tarjeta para ver, «sólo respaldo» dentro del ESSD, filtros Fecha/Sesión, Flujos → «Lo que reconoce» |
| 2 | Formatear la SD `MAVIC200` en el dron y reconectar | El trabajo de dron cierra por observación, como Sony |
| 3 | Popup al insertar | Medir latencia con la app cerrada y en menubar; aceptar o corregir |
| 4 | Cobertura ESSD contra MEDIA502 | `coverage --no-hash` como informe; liberar exige hash actual. [Política](docs/features/replication-policy.md) |

## P1 · escalabilidad del motor (en curso)

Objetivo: que agregar una cámara, un disco o un flujo sea **agregar datos a una receta**, no código, sin tocar la garantía plan → digest → revisión → copia verificada → limpieza.

| Paso | Trabajo | Estado |
| --- | --- | --- |
| 1 | Detección de tarjetas (Sony, H6, GoPro, Stalker, DJI, Canon…) y su recomendación como datos de la receta; las guardas de limpieza siguen en código | Hecho 8 nov (`a350af7f16af`) |
| 2 | La receta efectiva se expone por RPC (`recipe`) y Flujos muestra lo que reconoce | Hecho 8 nov. Falta generar los bloques de cada flujo desde la receta |
| 3 | Modelos por serial/EXIF en lugar de `ILCE-6400` fijo | Pendiente |
| 4 | Sobre común para intercambios con otras apps (productor, schema, digest, ACK) para handoff, `units.ndjson`, `cleanup-plan` y transcripciones | Diseño; [contratos](docs/contracts/README.md) |
| 5 | Recetas editables desde la app (frase → bloques) | Después de 1–3. [Diseño](docs/features/product-flows.md#flujos) |

## Motor en Rust · ruta

Orden revisado el 2026-11-11 a partir de la [auditoría](docs/audits/fable-audit-2026-11-10.md) y del código. Hoy Rust tiene 20 de unos 85 métodos (el camino de una tarjeta), sólo Windows tiene backend de volúmenes, la ventana de Windows no planea ni aplica, la app de Mac no usa Rust y ningún diferencial se ha ejecutado ([STATUS](docs/STATUS.md#por-área)). Las fases y reglas de fondo siguen en [rust-engine §7](docs/platforms/rust-engine.md#7-ruta-recomendada).

**Orden vigente desde el 2026-11-11: la Mac primero.** El usuario no tendrá un PC con Windows con este repo por un tiempo. Antes de dejar el PC quedaron hechos el paso 1, B6, el script de frontera y la parte de S3 sin diseño (LF y versiones fijadas). Los pasos 0 y 2 quedaron hechos en la Mac el 11 nov, con la CI y la sombra ([STATUS](docs/STATUS.md#entregas-recientes)). La app de Mac ya lee con Rust. Sigue completar los planes E2, las transferencias E3 con sus guardas y las acciones E4. La validación de la ventana usa el sandbox. La semana de uso diario y la prueba con hardware siguen abiertas. La Fase A queda en pausa. En paralelo, sin tocar el motor, la suite empieza por documentos y datos ([qué se muda y qué se queda](docs/suite-split.md)).

**Fase A · nüün usable en Windows (en pausa hasta tener Windows, salvo el paso 2).** Los pasos 1, 3, 4 y 5 necesitan Windows para probarse; el 2 necesita la Mac y sirve a las dos fases. El paso 1 sigue siendo condición antes de la primera copia real en Windows.

| Paso | Trabajo | Criterio de cierre | Brief |
| --- | --- | --- | --- |
| 1 | ~~Verificación sin caché en Windows~~ **Hecho el 2026-11-11** (`fs::verify_read`, [STATUS](docs/STATUS.md#entregas-recientes)). Falta verla en exFAT y con un lector SD real (en el paso 4), y compilar la ruta de Linux | Hecho: pruebas en NTFS, fila en el gate de [release](docs/release.md) | [S1](docs/audits/session-briefs-2026-11-10.md#s1--verificación-sin-caché-en-windows-y-linux) |
| 2 | ~~Dorado en la Mac con `TZ` fijo y respuestas guardadas en git~~ **Hecho el 2026-11-11**: 5 diferenciales verdes en la Mac, generador determinista (`--check` en CI). Las diferencias encontradas eran del arnés, no del motor | Hecho; falta correrlas en el PC sin Python (el archivo ya está en git) | [S2](docs/audits/session-briefs-2026-11-10.md#s2--dorado-en-la-mac-y-fixtures-en-git) |
| 3 | `plan_document` → revisión → `apply` → `resume` desde la ventana de Windows: proceso `rpc` por mutación, digest revisado, progreso, cancelación y reanudación. Quitar el `not_on_windows` de `RpcEngine` y su comentario viejo | Recorrido por UI Automation en raíz de QA con una carpeta local, incluida la reanudación tras cortar | Sin brief propio; [S9](docs/audits/session-briefs-2026-11-10.md#s9--integración-de-sistema-en-la-ventana-de-windows) lo da por hecho |
| 4 | Corrida física pedida por el usuario: una SD y un disco USB reales | Job con plan revisado, verificado y reanudado tras desconectar; tiempos por fase en STATUS | — |
| 5 | `eject_card`, confirmación de limpieza de cámara, `recover_catalog`, `capabilities`; aviso al terminar con «Expulsar» y «Ver resultado» | Pruebas portadas de Python; aviso según [avisos v1](docs/contracts/nu9ve-notifications-v1.md) | S9 |

**Fase B · Rust sustituye a Python en la Mac.**

| Paso | Trabajo | Criterio de cierre | Brief |
| --- | --- | --- | --- |
| 0 | ~~Compilar y probar el motor en macOS~~ **Hecho el 2026-11-11**: sin descargas, 110 pruebas, fmt y clippy limpios (clippy encontró un `///` que sólo compila en Unix). CI de GitHub para Mac y Windows escrita, sin correr hasta el push | Hecho | — |
| S | **Sombra.** `rpc-serve` de Python repite cada lectura en Rust después de responder y anota igual/distinto/`not_ported` y los tiempos ([media/shadow.py](media/shadow.py)); `scripts/engine_shadow.py replay` hace lo mismo sin la app. Herramientas hechas el 2026-11-11; primera repetición sobre el catálogo real: todo lo portado igual, salvo un error de orden en `projects` (corregido) | `engine_shadow.py enable` y una semana de uso normal; `report` sin `diff` en lo portado | [engine/README](engine/README.md#sombra-y-tiempos) |
| L | Portar las lecturas que faltan. **Hecho el 2026-11-12 lo que no depende de discos**: `project_preview`, `closure_overview`, `closure_link_candidates`, `closure_progress`, `action_progress`, `asset_revisions`, `living_collections`, `missions_status`, `integration_capabilities` (también en `snapshot`, en paralelo) y `capture_libraries`; iguales a Python en el dorado y en la Mac real ([tiempos](engine/README.md#sombra-y-tiempos)). Tras el paso 6 (12 nov) también `capture_overview`, `closure_deliveries` y `suggest`, y después `closure_candidates`, `missions_review` y `storage_*`. **Hecho el 2026-11-12: lecturas persistentes** iguales a Python en el dorado y en la Mac real | `report` con `same` en cada lectura; `not_ported` sólo donde falte el backend de volúmenes | — |
| 6 | ~~Backend de volúmenes de macOS en Rust~~ **Hecho para lectura el 2026-11-12**: `MacBackend` con las mismas fuentes que Python (`statfs`, `diskutil -plist`, `statvfs`), discos descritos en paralelo; con él, `volumes`, `source_cards`, `snapshot` completo, `capture_overview`, `closure_deliveries` y `suggest` son iguales a Python sobre los discos reales y 2–3× más rápidos ([tiempos](engine/README.md#sombra-y-tiempos)). En Mac el backend de Rust escribe en el sandbox. La corrida física sigue pendiente para activarlo en uso diario. `eject_card` usa `diskutil eject` en el sandbox | Hecho: unitarias del plist y del volumen del sistema, dorado de proyectos, sombra real. Pendiente: corrida física para abrir las escrituras | [S16](docs/audits/session-briefs-2026-11-10.md#s16--backend-de-volúmenes-de-macos-en-rust) |
| 7 | ~~La app de Mac elige motor por método~~ **Hecho para lecturas el 2026-11-12**, instalado según [STATUS](docs/STATUS.md#build-y-pruebas): `PersistentReader` pide primero a `nuun-engine rpc-serve` cada lectura que el motor declara (`engine_info.ported`) y Python responde lo que Rust no tiene o si el proceso falla; Ajustes → Diagnóstico elige «Rust con Python de respaldo» o «Python» y muestra quién respondió cada lectura. `build_media_app.py` mete el motor firmado en `Resources/backend/nuun-engine`. En el sandbox también usa las escrituras declaradas en `engine_info.writes` | 28 pruebas Swift. Falta el recorrido de la ventana con el sandbox, completar la semana de uso diario y aceptar el ingreso Rust con hardware | — |
| 8 | **Hecho en el sandbox de macOS**: escaneo y comparación del índice. La portabilidad de la exclusión de Clipper a Windows se valida por separado | Diferencial por familia. Pruebas físicas por dispositivo donde haya limpieza | [Ruta E2–E4](#motor-en-rust--ruta) |
| 9 | Una semana de uso real en Mac sin diferencias; retirar Python de `media/` (salvo `legacy-save` y descargas si no se portan) | [rust-engine §7](docs/platforms/rust-engine.md#7-ruta-recomendada), fase 5 | — |

**Escrituras en Rust · fases.** En el sandbox de Mac, Rust anuncia los 92 nombres del servicio y del índice de discos. Quedan variantes pendientes, como el descubrimiento de sesiones Stalker y los planes Stalker con zona IANA explícita. `coverage` ya compara y guarda el informe en el catálogo local. El orden va de menor a mayor riesgo, y cada fase cierra con su dorado (estado del catálogo y de los archivos después de la operación, contra Python), sombra o repetición donde aplique, y entrada en STATUS:

| Fase | Métodos | Riesgo | Cierre |
| --- | --- | --- | --- |
| E0 · lecturas de un solo uso | **Hecho el 2026-11-12**: `finals_status`, `captures`, `publications`, `retitle_operations`, `collection_release_plans`, `resolve`, `handoff`, `sources`, `capture_discard_check` (con `clipper.inspect` portado, la guarda que también usarán liberar y cerrar). La app pide a Rust las lecturas declaradas. En uso diario, `discover` y `capture_discard_check` siguen en Python. El sandbox prueba `discover` de tarjetas con Rust. `coverage` genera su informe en Rust con y sin SHA-256 | Ninguno: sólo leen | Hecho: dorados (incluido un Clipper de prueba con proyecto antiguo, registro y cola) y repetición real iguales a Python |
| E1 · configuración del catálogo (nunca media) | **E1a hecho el 2026-11-12**: `defaults`/`integration_settings` con parámetros, `dismiss_job`, `touch_project`, `set_project_state`, `rename_project`, `set_work_state`, `set_publication`, `save_capture_tag(s)`, `set_finals_folder`, `cancel_plan`, `capture_discarded`, `closure_link`, `save_profile`, `confirm_device` (`settings.rs`; el lock donde Python lo toma). La app las usa con Rust en el sandbox. **E1b implementado**: `closure_save`, `storage_add_folder`/`storage_remove_folder` y registro en el sandbox. Activar el registro de uso diario sigue sujeto a la aceptación | Bajo: sólo el catálogo, bajo el mismo lock | Hecho E1a: dorado de 45 escrituras (válidas y rechazadas) con el mismo catálogo final que Python, tabla por tabla |
| E2 · planes revisados | Portados `plan_release`, `review_release`, `plan_finals`/`plan_finals_document` y las variantes Stalker y entrega de `plan`/`plan_document`. Los manifiestos Stalker con zona IANA explícita conservan el respaldo Python. `closure_plan` cubre el cierre editorial completo y los finales publicados; `dng_plan` cubre selección explícita y automática. Portados `collection_release_plan` y `living_plan` | Bajo: escriben el plan y su digest | Diferencial de planes de liberación, Stalker y finales. Faltan más casos por variante |
| E3 · transferencias, liberación e índice | Portados `deliver_work`, `restore`, `recover_catalog`, `transcripts_backup`, `coverage` y el retiro con exclusión de Clipper (`apply_release`, `relink_release`, liberación automática de Stalker y limpieza de finales en el sandbox de Mac). `closure_apply` cubre el cierre editorial y los finales publicados. Portados `relayout`, `retitle_capture` y `publish_revision` para Stalker. Portados `missions_incorporate` y `dng_apply` en el sandbox de Mac. Portados `collection_release_apply` y `living_apply`. Portados `storage_scan` y `storage_compare` en el sandbox de Mac, con publicación atómica del índice y reuso de hashes | Alto | Pruebas de interrupción y reintento, comparación de recibos y bytes, y aceptación en el sandbox |
| E4 · dispositivo | Portados `eject_card`, `confirm_camera_cleanup` y `reconcile_sony_cleanup` en el sandbox de Mac | Alto | Fixtures de identidad, bloqueo, Sony e índices. Falta corrida física explícita con una tarjeta ([release](docs/release.md)) |

**Sandbox de macOS · implementado.** [Guía y comandos](engine/README.md#sandbox-de-macos). Siguiente aceptación de Ingreso en la ventana: revisar un plan Sony, aplicar en Rust, pausar/reanudar y ver el estado de cámara. Después, validar con una tarjeta y un destino USB de prueba, incluyendo desconexión y relectura en exFAT. Para el resto de la migración, seguir E2 → E3 → E4 y mantener el diferencial por familia.

**Decidido por el usuario (2026-11-11): lo nuevo entra en Rust.** Python sólo recibe correcciones. La excepción es una función que desbloquea un flujo de uso diario y que Rust no puede llevar a la app a tiempo: entra en Python con su caso en el dorado, para portarla después. En Mac, lo nuevo en Rust llega a la app por el paso 7 (por método) y, si necesita discos, después del paso 6. Cada lectura portada se mide con la sombra para documentar cuánto cambia el tiempo.

## P2 · siguientes flujos de producto

| Área | Pendiente real | Referencia |
| --- | --- | --- |
| **Música y SSD1** | Orden decidido en [music-flows](docs/features/music-flows.md): (1) liberar `recordingslong`/`recordings` (~252 GB) contra MEDIA500/501 por hash, conservando lo que usa un set (`.asd` o `.als`, 10.5 GB) — **código hecho 9 nov, falta la corrida real** ([runbook](docs/features/music-flows.md#m1--liberar-una-colección-implementado-2026-11-09)); (2) carpeta viva para producción y `~/Music` — **código hecho 9 nov, falta elegir carpetas y la primera corrida** ([runbook](docs/features/music-flows.md#m2--carpeta-viva-implementado-2026-11-09)); (3) triage de audio con Clipper (`audio-triage-v1`); (4) biblioteca con curado (`music-library-v1`) | [Inventario](docs/features/music-ssd1-inventory-2026-11-09.md) |
| Dron, resto | Reclasificar `pending_export` → misión al llegar el export; confirmar dispositivo nuevo en la app; UI del plan de DNG | [Atlas](docs/contracts/atlas-drone-v1.md#estado-del-lado-nüün--2026-11-08) |
| Selector por proyecto | Todas las fuentes y capturas pendientes de un proyecto, con un plan por fuente | Pedido del 6 nov |
| Fotos / Kyū | Consumidor de `camera_bundle` RAW+JPG | [Fotos](docs/features/product-flows.md#fotos-y-kyū) |
| Audio / cámaras | H6/SSD1, DJI 360/acción, GoPro con evidencia real | Por dispositivo |
| Stalker | Casos reales `recording`/`finalizing`/`recovery` | [Pedidos](docs/contracts/README.md#pendientes-para-n-creator-utils) |
| Estado humano | Restaurar sin SD, retención de snapshots, rehash periódico | Catálogo/library/recovery |

**Clipper:** sources/handoff, relink y ACK ya adoptados; `library retire-plan`/`retire` existe del lado de Clipper. Que nüün lo invoque sería otra entrega.

## P3 · robustez y distribución

- Digest de plan autenticado o reverificación en `apply`; owner/lease de jobs.
- Automatizar flujos ya probados (`preparar` al conectar, luego sólo copias). Limpiar y liberar siempre piden confirmación.
- Medir antes de optimizar: bibliotecas grandes, miniaturas, idle CPU/IO, DiskArbitration.
- Distribución: runtime empaquetado, versiones alineadas, Developer ID, notarización. [Gates](docs/release.md).
- MCP con mutaciones, Linux, nube y segunda réplica. Descargas/stems como derivados; MIDI, iluminación e IoT exploratorios.
- **Windows** (maqueta hecha 2026-11-09, [guía](docs/platforms/windows.md)). Siguiente, en orden:
  1. ~~Decidir dónde viven `Nu9ve.Core`/`Nu9ve.WinUI`~~: decidido el 2026-11-11, repo `nu9ve/suite` con copia fijada ([S5](docs/audits/session-briefs-2026-11-10.md#s5--decidir-y-crear-el-repo-de-la-suite)); falta crearlo.
  2. Que `ThemeRolesTests.swift` lea los vectores compartidos.
  3. ~~Adaptador de volúmenes de Windows en `media/` y Python empaquetado~~: ya no hace falta; el adaptador existe en Rust y Windows usa el motor de Rust. Lo que sigue es la [Fase A del motor](#motor-en-rust--ruta).
  4. Firma e instalación del MSIX y del Setup en una máquina limpia.
- **Motor en Rust: adoptado por el usuario el 2026-11-09.** [Análisis](docs/platforms/rust-engine.md). Razones: distribución sin Python, un motor para Windows y Mac, piezas compartidas con nove y curado. **No** la velocidad de verificación (medido: SHA-256 a 1,67 GB/s en Rust y .NET; el disco manda). Python es el oráculo hasta la paridad de cada método. Orden de trabajo en [Motor en Rust · ruta](#motor-en-rust--ruta).
- **MCP** ([contrato v1](docs/contracts/nu9ve-mcp-v1.md)): que `media mcp` pida al lector persistente en vez de abrir el catálogo, `serverInfo` `nuun`, campos `_untrusted`, `show_in_app` para un proyecto o un plan revisado y `docs/mcp.md`. Nunca copiar, limpiar ni liberar por MCP.

## Auditoría del 2026-11-10 (propuestas, sin priorizar aquí)

La [auditoría Fable](docs/audits/fable-audit-2026-11-10.md) revisó nüün, nove, curado y Fractos en el PC de Windows y propone hallazgos con [briefs por sesión](docs/audits/session-briefs-2026-11-10.md). Un hallazgo entra a una tabla de arriba cuando el usuario lo elige. Ya entraron (2026-11-11) a la [ruta del motor](#motor-en-rust--ruta): verificación sin caché ([A1](docs/audits/fable-audit-2026-11-10.md#a1--la-relectura-de-verificación-no-evita-la-caché-fuera-de-macos), paso 1), dorado en git ([B2](docs/audits/fable-audit-2026-11-10.md#b2--no-hay-ci-y-el-diferencial-nunca-ha-corrido), paso 2) y backend de macOS ([B6](docs/audits/fable-audit-2026-11-10.md#b6--el-motor-decide-por-cfgwindows), paso 6). De [S3](docs/audits/session-briefs-2026-11-10.md#s3--higiene-de-repos), en este repo quedaron hechos el 2026-11-11 `.gitattributes`, versiones fijadas, README y `windows.md` §1; faltan los enlaces con rutas de una Mac (C6) y los otros repos.

Decidido por el usuario el 2026-11-10: sonido de interfaz **encendido por defecto, con una opción clara para silenciar**, en toda la suite ([sonido v1 §7](docs/contracts/nu9ve-sound-v1.md#7-valor-por-defecto-ajustes-y-formato)).

Decidido el 2026-11-11: lo compartido de la suite vivirá en un repo `nu9ve/suite` y cada app llevará una **copia fijada** ([cómo](docs/suite-sharing.md)); sin implementar, empieza por el [brief S5](docs/audits/session-briefs-2026-11-10.md#s5--decidir-y-crear-el-repo-de-la-suite). Esto responde el paso 1 de Windows en P3 (dónde viven `Nu9ve.Core`/`Nu9ve.WinUI`). Inventario por archivo de lo que se muda, cuándo y lo que se queda: [suite-split](docs/suite-split.md). En resumen: documentos y datos ya; el código de Rust después de que el motor funcione en la Mac; Swift tras decidir `NuMacShell`; C# cuando haya Windows.

Decisiones que esperan: canal de firma y actualización, cuarentena o papelera al liberar, identidad de volumen portable. Para nüün en Mac queda propuesto el estándar de [avisos del sistema](docs/contracts/nu9ve-notifications-v1.md): hoy sólo avisa de una tarjeta detectada.

## Prioridad baja · nombre interno

Renombrar `MediaMenu` → `Nuun` (paquete, targets, ejecutable, prefijo `Nu9veMedia-local-*`, scripts y docs) en un cambio propio sin comportamiento nuevo. Se conservan `CFBundleIdentifier` `app.nu9ve.media`, `~/Applications/nuun.app` y el catálogo.

## Retomar en otra sesión

Leer CONTEXT → STATUS (tabla por área) → fila de este PLAN → contrato o módulo dueño. Registrar sólo resultados observados con job/build; no heredar aprobación física de otra build o de fixtures. Respetar `git status`, sin commits/restage ni mensajes a otras sesiones como efecto lateral. [Formato de handoff](docs/development.md#cerrar-y-retomar-una-sesión).
