# Plan de trabajo

Actualizado: 2026-11-26, America/Hermosillo. Éste es el backlog priorizado. [STATUS](docs/STATUS.md) dice qué funciona hoy y con qué evidencia; [CONTEXT](docs/CONTEXT.md) guarda las decisiones. La historia por entrega está en [releases/](docs/releases/status-log-2026-10-25_2026-11-08.md) y no es otra lista de pendientes.

## Cerrado (no reabrir sin un hecho nuevo)

- **Sony:** retiro de clips verificados, índices conservados y cierre al observar el formato. El ingreso físico `17e70e69` reprodujo una entrada aún listada tras retirar el archivo. Se corrigió la lectura final. El retiro se comprobó tras remontar la Sony del ensayo físico. [STATUS](docs/STATUS.md#lectura-final-sony--2026-11-13)
- **Dron, ingreso completo:** una SD del Mavic se respalda, se verifica, se copia el trabajo sin duplicados y queda vacía, siempre que el export de subaereo ya esté incorporado. Si falta, A14 bloquea la limpieza a propósito.
- **Cierre de Terra:** aplicado el 8 nov.

## P0 · uso diario sin sorpresas

| Orden | Trabajo | Criterio de cierre |
| --- | --- | --- |
| 1 | Aceptar en la app la build `a350af7f16af` | Tarjeta para ver, «sólo respaldo» dentro del ESSD, filtros Fecha/Sesión, Flujos → «Lo que reconoce» |
| 2 | Formatear la SD `MAVIC200` en el dron y reconectar | El trabajo de dron cierra por observación, como Sony |
| 3 | Popup al insertar | Medir latencia con la app cerrada y en menubar; aceptar o corregir |
| 4 | Cobertura ESSD contra MEDIA502 | Informe por tamaño hecho para `001_proyectos/e64_raw`; falta ampliar al resto del ESSD. Liberar exige hash actual. [Política](docs/features/replication-policy.md) |
| 5 | Retiro de raws del ESSD con un plan revisado | Debloateando mi tele aplicado (8,14 GB). Próximo: revisar 16,35 GB entre Baches con el dron y El repositorio de nu9ve, con reenlaces de Clipper. [Evidencia](docs/STATUS.md#diagnóstico-de-essd-y-música--2026-11-13) |

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

**Orden vigente: macOS primero.** La instalación local `e70e525ae792` usa Rust para el ingreso y demás operaciones compatibles. Python queda disponible en Diagnóstico. El sandbox declara 92 métodos de servicio/índice y pasó ingreso Sony con pausa y reanudación, Stalker, respaldo de publicados y mapa de discos en ventana. Evidencia en [STATUS](docs/STATUS.md#aceptación-local-de-macos--2026-11-13). El ensayo físico pasó. Queda la semana de uso diario. La fase A sigue en el PC con Windows: pasos 1 a 3 hechos el 16 nov. [Ruta de fondo](docs/platforms/rust-engine.md#7-ruta-recomendada).

**Fase A · nüün usable en Windows (retomada el 2026-11-16 en el PC).** Hechos los pasos 1 a 3. Sigue el 4, una corrida física que pide el usuario; el paso 1 queda por comprobar en exFAT y con un lector real dentro de esa corrida.

| Paso | Trabajo | Criterio de cierre | Brief |
| --- | --- | --- | --- |
| 1 | ~~Verificación sin caché en Windows~~ **Hecho el 2026-11-11** (`fs::verify_read`, [STATUS](docs/STATUS.md#entregas-recientes)). Falta verla en exFAT y con un lector SD real (en el paso 4), y compilar la ruta de Linux | Hecho: pruebas en NTFS, fila en el gate de [release](docs/release.md) | [S1](vendor/nu9ve-core/docs/audits/session-briefs-2026-11-10.md#s1--verificación-sin-caché-en-windows-y-linux) |
| 2 | ~~Dorado en la Mac con `TZ` fijo y respuestas guardadas en git~~ **Hecho el 2026-11-11**: 5 diferenciales verdes en la Mac, generador determinista (`--check` en CI). Las diferencias encontradas eran del arnés, no del motor | Hecho; falta correrlas en el PC sin Python (el archivo ya está en git) | [S2](vendor/nu9ve-core/docs/audits/session-briefs-2026-11-10.md#s2--dorado-en-la-mac-y-fixtures-en-git) |
| 3 | ~~`plan_document` → revisión → `apply` → `resume` desde la ventana de Windows~~ **Hecho el 2026-11-16**: trabajador `rpc` por mutación, digest revisado, progreso desde `job`, pausa cooperativa con un evento con nombre y reanudación en Resultado y en Actividad ([STATUS](docs/STATUS.md#ingreso-desde-la-ventana-de-windows--2026-11-16)) | Hecho: recorrido por UI Automation en raíz de QA con carpetas locales, con pausa y con el trabajador cortado, y cotejo SHA-256 | Sin brief propio; [S9](vendor/nu9ve-core/docs/audits/session-briefs-2026-11-10.md#s9--integración-de-sistema-en-la-ventana-de-windows) lo da por hecho |
| 4 | Corrida física pedida por el usuario: una SD y un disco USB reales | Job con plan revisado, verificado y reanudado tras desconectar; tiempos por fase en STATUS | — |
| 5 | `eject_card`, confirmación de limpieza de cámara, `recover_catalog`, `capabilities`; aviso al terminar con «Expulsar» y «Ver resultado» | Pruebas portadas de Python; aviso según [avisos v1](vendor/nu9ve-core/contracts/nu9ve-notifications-v1.md) | S9 |

**Abierto con `nu9ve/core` (2026-11-16).** La pausa de Windows usa un evento con nombre (`NUUN_CANCEL_EVENT`). La §6.2 de [nu9ve-engine-v1](vendor/nu9ve-core/contracts/nu9ve-engine-v1.md#62-progreso-y-cancelación) pide un método o un archivo de cancelación, con plazo de gracia y `TerminateJobObject`. Falta que el usuario decida si el contrato acepta el evento o si nüün pasa a un archivo. En los dos casos, la ventana tiene que arrancar el trabajador en un Job Object y cortarlo tras el plazo de gracia. La propuesta, junto con lo que nüün y nove repiten en Windows (cliente de motor en C#, avisos, scripts de QA, certificado de desarrollo, barra de tareas), está en `nu9ve/core`: `docs/windows-shared-next.md`. Conviene resolverla antes del paso 4. Los avisos y la barra de tareas del paso 5 deberían nacer allá.

**Fase B · Rust sustituye a Python en la Mac.**

| Paso | Trabajo | Criterio de cierre | Brief |
| --- | --- | --- | --- |
| 0 | ~~Compilar y probar el motor en macOS~~ **Hecho el 2026-11-11**: sin descargas, 110 pruebas, fmt y clippy limpios (clippy encontró un `///` que sólo compila en Unix). CI de GitHub para Mac y Windows escrita, sin correr hasta el push | Hecho | — |
| S | **Sombra.** `rpc-serve` de Python repite cada lectura en Rust después de responder y anota igual/distinto/`not_ported` y los tiempos ([media/shadow.py](media/shadow.py)); `scripts/engine_shadow.py replay` hace lo mismo sin la app. Herramientas hechas el 2026-11-11; primera repetición sobre el catálogo real: todo lo portado igual, salvo un error de orden en `projects` (corregido). Estricta en el tipo y sin valores en el registro desde el 2026-11-16 ([nu9ve-engine-v1](vendor/nu9ve-core/contracts/nu9ve-engine-v1.md#10-modo-sombra-contra-un-oráculo)); falta repetirla así sobre el catálogo real | `engine_shadow.py enable` y una semana de uso normal; `report` sin `diff` en lo portado | [engine/README](engine/README.md#sombra-y-tiempos) |
| L | Portar las lecturas que faltan. **Hecho el 2026-11-12 lo que no depende de discos**: `project_preview`, `closure_overview`, `closure_link_candidates`, `closure_progress`, `action_progress`, `asset_revisions`, `living_collections`, `missions_status`, `integration_capabilities` (también en `snapshot`, en paralelo) y `capture_libraries`; iguales a Python en el dorado y en la Mac real ([tiempos](engine/README.md#sombra-y-tiempos)). Tras el paso 6 (12 nov) también `capture_overview`, `closure_deliveries` y `suggest`, y después `closure_candidates`, `missions_review` y `storage_*`. **Hecho el 2026-11-12: lecturas persistentes** iguales a Python en el dorado y en la Mac real | `report` con `same` en cada lectura; `not_ported` sólo donde falte el backend de volúmenes | — |
| 6 | ~~Backend de volúmenes de macOS en Rust~~ **Hecho para lectura el 2026-11-12**: `MacBackend` con las mismas fuentes que Python (`statfs`, `diskutil -plist`, `statvfs`), discos descritos en paralelo; con él, `volumes`, `source_cards`, `snapshot` completo, `capture_overview`, `closure_deliveries` y `suggest` son iguales a Python sobre los discos reales y 2–3× más rápidos ([tiempos](engine/README.md#sombra-y-tiempos)). En Mac el backend de Rust escribe en el sandbox. La corrida física sigue pendiente para activarlo en uso diario. `eject_card` usa `diskutil eject` en el sandbox | Hecho: unitarias del plist y del volumen del sistema, dorado de proyectos, sombra real. Pendiente: corrida física para abrir las escrituras | [S16](vendor/nu9ve-core/docs/audits/session-briefs-2026-11-10.md#s16--backend-de-volúmenes-de-macos-en-rust) |
| 7 | ~~La app de Mac elige motor por método~~ **Hecho para lecturas el 2026-11-12**, instalado según [STATUS](docs/STATUS.md#build-y-pruebas): `PersistentReader` pide primero a `nuun-engine rpc-serve` cada lectura que el motor declara (`engine_info.ported`) y Python responde lo que Rust no tiene o si el proceso falla; Ajustes → Diagnóstico elige «Rust con Python de respaldo» o «Python» y muestra quién respondió cada lectura. `build_media_app.py` mete el motor firmado en `Resources/backend/nuun-engine`. En el sandbox también usa las escrituras declaradas en `engine_info.writes` | 28 pruebas Swift. Recorrido en ventana con imágenes completado el 13 nov. Pendientes semana de uso diario y ensayo físico | — |
| 8 | **Hecho en el sandbox de macOS**: escaneo y comparación del índice. La portabilidad de la exclusión de Clipper a Windows se valida por separado | Diferencial por familia. Pruebas físicas por dispositivo donde haya limpieza | [Ruta E2–E4](#motor-en-rust--ruta) |
| 9 | Una semana de uso real en Mac sin diferencias; retirar Python de `media/` (salvo `legacy-save` y descargas si no se portan) | [rust-engine §7](docs/platforms/rust-engine.md#7-ruta-recomendada), fase 5 | — |

**Escrituras en Rust · fases.** En el sandbox de Mac, Rust anuncia los 92 nombres del servicio y del índice de discos. También están portados el descubrimiento de sesiones Stalker y los planes Stalker con zona IANA explícita. La aceptación en ventana con imágenes pasó el 13 nov. El ensayo físico sigue pendiente. `coverage` ya compara y guarda el informe en el catálogo local. El orden va de menor a mayor riesgo, y cada fase cierra con su dorado (estado del catálogo y de los archivos después de la operación, contra Python), sombra o repetición donde aplique, y entrada en STATUS:

| Fase | Métodos | Riesgo | Cierre |
| --- | --- | --- | --- |
| E0 · lecturas de un solo uso | **Hecho el 2026-11-12**: `finals_status`, `captures`, `publications`, `retitle_operations`, `collection_release_plans`, `resolve`, `handoff`, `sources`, `capture_discard_check` (con `clipper.inspect` portado, la guarda que también usarán liberar y cerrar). La app pide a Rust las lecturas declaradas. `discover` usa Rust con el motor seleccionado. `capture_discard_check` usa Python. El sandbox prueba `discover` de tarjetas y sesiones Stalker con Rust. `coverage` genera su informe en Rust con y sin SHA-256 | Ninguno: sólo leen | Hecho: dorados (incluido un Clipper de prueba con proyecto antiguo, registro y cola) y repetición real iguales a Python |
| E1 · configuración del catálogo (nunca media) | **E1a hecho el 2026-11-12**: `defaults`/`integration_settings` con parámetros, `dismiss_job`, `touch_project`, `set_project_state`, `rename_project`, `set_work_state`, `set_publication`, `save_capture_tag(s)`, `set_finals_folder`, `cancel_plan`, `capture_discarded`, `closure_link`, `save_profile`, `confirm_device` (`settings.rs`; el lock donde Python lo toma). La app las usa con Rust en el sandbox. **E1b implementado**: `closure_save`, `storage_add_folder`/`storage_remove_folder` y registro en el sandbox. Activar el registro de uso diario sigue sujeto a la aceptación | Bajo: sólo el catálogo, bajo el mismo lock | Hecho E1a: dorado de 45 escrituras (válidas y rechazadas) con el mismo catálogo final que Python, tabla por tabla |
| E2 · planes revisados | Portados `plan_release`, `review_release`, `plan_finals`/`plan_finals_document` y las variantes Stalker y entrega de `plan`/`plan_document`. La evidencia de reloj Stalker usa las reglas IANA en el instante de captura. `closure_plan` cubre el cierre editorial completo y los finales publicados; `dng_plan` cubre selección explícita y automática. Portados `collection_release_plan` y `living_plan` | Bajo: escriben el plan y su digest | Diferencial de planes de liberación, Stalker y finales. Faltan más casos por variante |
| E3 · transferencias, liberación e índice | Portados `deliver_work`, `restore`, `recover_catalog`, `transcripts_backup`, `coverage` y el retiro con exclusión de Clipper (`apply_release`, `relink_release`, liberación automática de Stalker y limpieza de finales en el sandbox de Mac). `closure_apply` cubre el cierre editorial y los finales publicados. Portados `relayout`, `retitle_capture` y `publish_revision` para Stalker. Portados `missions_incorporate` y `dng_apply` en el sandbox de Mac. Portados `collection_release_apply` y `living_apply`. Portados `storage_scan` y `storage_compare` en el sandbox de Mac, con publicación atómica del índice y reuso de hashes | Alto | Pruebas de interrupción y reintento, comparación de recibos y bytes, y aceptación en el sandbox |
| E4 · dispositivo | Portados `eject_card`, `confirm_camera_cleanup` y `reconcile_sony_cleanup` en el sandbox de Mac | Alto | Fixtures de identidad, bloqueo, Sony e índices. Falta corrida física explícita con una tarjeta ([release](docs/release.md)) |

**Sandbox de macOS.** [Guía y comandos](engine/README.md#sandbox-de-macos). Los casos de ventana y sus verificaciones se registran en [STATUS](docs/STATUS.md#aceptación-local-de-macos--2026-11-13).

### Ensayo físico de macOS

1. **Hecho el 13 nov.** Copia sintética a MEDIA502 en ExFAT y respaldo con limpieza de un clip prescindible de la Sony real. Se usó una carpeta exclusiva de prueba y un catálogo temporal con rutas y UUID autorizados. [Evidencia](docs/STATUS.md#ingreso-físico-sony-con-rust--2026-11-13)
2. **Remontaje comprobado.** Los tres archivos retirados siguieron ausentes, los nueve índices conservaron sus hashes y Rust informó `waiting_camera`.
3. **Extracción y reanudación comprobadas.** La desconexión dejó cero retiros. Tras corregir el cambio de inode del marcador Sony vacío, el mismo job reanudó la copia, verificó los 12 archivos y retiró los tres miembros autorizados. Falta observar el cierre por formato y completar la semana de uso diario. [Evidencia](docs/STATUS.md#reanudación-física-sony-con-rust--2026-11-13)

**Decidido por el usuario (2026-11-11): lo nuevo entra en Rust.** Python sólo recibe correcciones. La excepción es una función que desbloquea un flujo de uso diario y que Rust no puede llevar a la app a tiempo: entra en Python con su caso en el dorado, para portarla después. En Mac, lo nuevo en Rust llega a la app por el paso 7 (por método) y, si necesita discos, después del paso 6. Cada lectura portada se mide con la sombra para documentar cuánto cambia el tiempo.

## P2 · siguientes flujos de producto

| Área | Pendiente real | Referencia |
| --- | --- | --- |
| **Música y SSD1** | (1) Primera revisión de `recordingslong` 2019–2021 al conectar SSD1, MEDIA500/501 y ESSD; conservar lo que usa un set (`.asd` o `.als`). (2) Carpeta viva: piloto `~/Music/03_final` completado con Rust y segundo plan vacío. Abrir el set respaldado en Ableton y elegir la siguiente carpeta de producción. (3) Triage con Clipper (`audio-triage-v1`). (4) Biblioteca con curado (`music-library-v1`). | [Runbooks](docs/features/music-flows.md#fases) · [Evidencia](docs/STATUS.md#diagnóstico-de-essd-y-música--2026-11-13) |
| Dron, resto | Asociación revisada de fotos `pending_export` en catálogo implementada; falta el traslado journalizado a misiones y actualizar mapas si cambian rutas. Confirmar dispositivo nuevo y UI de DNG | [Evidencia DJI](docs/STATUS.md#misiones-dji-al-incorporar-un-export--2026-11-13), [Atlas](docs/contracts/atlas-drone-v1.md#estado-del-lado-nüün--2026-11-08) |
| A77 · exports de subaereo | nüün responde con `missions_ingest` por su socket local en macOS. Pendiente una entrega de punta a punta cuando nove implemente el cliente | [Contrato de entrega](docs/contracts/atlas-drone-v1.md#respuesta-de-nüün-a-a77-2026-11-26) |
| Conversaciones en Inicio | Actualización de la copia desde esta Mac al SSD por manifiesto, con confirmación. Siguiente: aceptar en ventana y decidir cómo avisar de cambios entre dos SSD | [Flujo](docs/features/suite-disks-flows.md#3-sugerencias-al-conectar) |
| Poner al día | Borrador recuperable, copias de edición y carpetas vivas registradas en el lote Rust. DJI incorpora y compara dentro del lote. Siguiente: aceptación en ventana al desconectar y reconectar y progreso por paso | [Flujos y contrato](docs/features/save-all.md) |
| Bandeja por proyecto | Primer corte en macOS: ubicaciones catalogadas, Stalker pendiente, entregas candidatas, requisitos de cierre y acceso a la revisión de raws. Faltan fuentes SD asignadas y aceptación en ventana | [Proyectos](docs/features/nuun-window.md#bandeja-por-proyecto) |
| Fotos / Kyū | Consumidor de `camera_bundle` RAW+JPG. Canon SL3 ya tiene evidencia de detección y plan con JPG y MP4. Falta probar RAW+JPG y ejecutar una copia física de muestra. | [Fotos](docs/features/product-flows.md#fotos-y-kyū) · [Estado por dispositivo](docs/STATUS.md#por-área) |
| Audio / cámaras | H6: WAV/HPRJ agrupados, reloj BWF y archivos ajenos separados en vista previa real. Confirmar SSD1 antes de aplicar. DJI 360/acción y GoPro requieren evidencia física. Insta360 X3: plan físico de INSV y fotos comprobado en Rust. Falta copiar y abrir una muestra en Studio y resolver FreeFrame. | [Estado por dispositivo](docs/STATUS.md#por-área) |
| Handycam | Histórico principal consolidado por Poner al día y conversión física con expulsión comprobada. Validar un lote mixto en ventana. Limpiar sólo desde el equipo después de comprobar cobertura. | [Contrato y pasos](docs/contracts/ingest-flow-v1.md#hdd-de-handycam-por-usb) |
| Stalker | Casos reales `recording`/`finalizing`/`recovery` | [Pedidos](docs/contracts/README.md#pendientes-para-n-creator-utils) |
| Colecciones (Agentes) | Lectura `storage_collections` hecha en los dos motores, con medición real el 2026-11-19. Pendientes: (1) dar acceso en el sandbox o leer por el motor fuera de él antes de mostrarla en Discos; (2) plan revisado para retirar cachés, nunca automático; (3) servirla por `nu9ve-data-v1` cuando el core lo publique; (4) clasificar lo que aún sale como `other`, como `computer-use` y `sqlite` de Codex | [Colecciones](docs/features/storage-map.md#colecciones) · [Evidencia](docs/STATUS.md#colección-agentes--2026-11-19) |
| **Acumulados en disco** (pedido del usuario, 2026-11-25) | Fase 1 hecha (inventario y plan en sólo lectura, CLI, RPC, Discos → Acumulados…, dataset de la suite). Siguen en orden: (1) un escaneo real en sólo lectura de `~/code` y las cachés, pedido por el usuario, y aceptar la vista en ventana; (2) **fase 2**: ejecutar el plan aprobado por grupo, a la Papelera por omisión, dos confirmaciones para lo que no se regenera, volver a medir antes de mover y recibo con lo liberado según `df`; (3) actividad de agentes desde nove (`agents:summary:sessions`) en lugar del mtime de `~/.claude/projects`; (4) acceso del sandbox a `HOME` | [Diseño](docs/features/storage-map.md#acumulados) · [Evidencia](docs/STATUS.md#perfiles-y-acumulados--2026-11-25) |
| **Perfiles de la suite** | Lectura y registro hechos en el motor y en Ajustes → Perfiles. Falta registrar los perfiles reales en la M1 (`e64`, `nu9ve-xyz`), pedido por el usuario, y decidir el tipo de `nu9ve-xyz` (hoy `team`) | [nu9ve-profile-v1](vendor/nu9ve-core/contracts/nu9ve-profile-v1.md) · [STATUS](docs/STATUS.md#perfiles-y-acumulados--2026-11-25) |
| Benchmarks | `scripts/engine_bench.py` (golden, walk, soak) en `bench/nuun.ndjson`. Falta el caso de copias (GiB/s) sobre fixture, y meter `soak --check-soak` en el gate de release | [nu9ve-bench-v1](vendor/nu9ve-core/contracts/nu9ve-bench-v1.md) |
| Estado humano | Restaurar sin SD, retención de snapshots, rehash periódico | Catálogo/library/recovery |

**Clipper:** sources/handoff, relink y ACK ya adoptados; `library retire-plan`/`retire` existe del lado de Clipper. Que nüün lo invoque sería otra entrega.

## P3 · robustez y distribución

- **Identidad portable F0:** registro, publicación y adopción de sólo lectura, aceptados en ventana con MEDIA502 y ESSD; `nu9ve-volumes` ya vive en nu9ve/core. Falta: la otra Mac y Windows ([F0 → F1](#suite-de-f0-a-f1)). [Estado](docs/STATUS.md#identidad-portable-f0--2026-11-22).
- Digest de plan autenticado o reverificación en `apply`; owner/lease de jobs.
- Automatizar flujos ya probados (`preparar` al conectar, luego sólo copias). Limpiar y liberar siempre piden confirmación.
- Medir antes de optimizar: bibliotecas grandes, miniaturas, idle CPU/IO, DiskArbitration.
- Distribución: runtime empaquetado, versiones alineadas, Developer ID, notarización. [Gates](docs/release.md).
- MCP con mutaciones, Linux, nube y segunda réplica. Descargas/stems como derivados; MIDI, iluminación e IoT exploratorios.
- **Windows** (maqueta hecha 2026-11-09, [guía](vendor/nu9ve-core/docs/platforms/windows.md)). Siguiente, en orden:
  1. ~~Decidir dónde viven `Nu9ve.Core`/`Nu9ve.WinUI`~~: decidido el 2026-11-11, repo `nu9ve/core` con copia fijada ([S5](vendor/nu9ve-core/docs/audits/session-briefs-2026-11-10.md#s5--decidir-y-crear-el-repo-de-la-suite)); creado en local el 2026-11-16 con los documentos. `Nu9ve.Core`/`Nu9ve.WinUI` se mudaron el 2026-11-16 a `nu9ve/core` `windows/`; nüün los usa desde su copia fijada y declara su identidad en `Nuun.Core/Theme/NuunTheme.cs`.
  2. Que `ThemeRolesTests.swift` lea los vectores compartidos.
  3. ~~Adaptador de volúmenes de Windows en `media/` y Python empaquetado~~: ya no hace falta; el adaptador existe en Rust y Windows usa el motor de Rust. Lo que sigue es la [Fase A del motor](#motor-en-rust--ruta).
  4. Firma e instalación del MSIX y del Setup en una máquina limpia.
- **Motor en Rust: adoptado por el usuario el 2026-11-09.** [Análisis](docs/platforms/rust-engine.md). Razones: distribución sin Python, un motor para Windows y Mac, piezas compartidas con nove y curado. **No** la velocidad de verificación (medido: SHA-256 a 1,67 GB/s en Rust y .NET; el disco manda). Python es el oráculo hasta la paridad de cada método. Orden de trabajo en [Motor en Rust · ruta](#motor-en-rust--ruta).
- **MCP** ([contrato v1](vendor/nu9ve-core/contracts/nu9ve-mcp-v1.md)): que `media mcp` pida al lector persistente en vez de abrir el catálogo, `serverInfo` `nuun`, campos `_untrusted`, `show_in_app` para un proyecto o un plan revisado y `docs/mcp.md`. Nunca copiar, limpiar ni liberar por MCP.

## Suite: de F0 a F1

Plan de [varias máquinas](vendor/nu9ve-core/docs/suite-devices.md). F0 está [en STATUS](docs/STATUS.md#identidad-portable-f0--2026-11-22). Esto es lo que falta, en orden. F1 empezó el 2026-11-23 con el agente.

### Cerrar F0

1. ~~**Índice por proyecto**~~ **Hecho el 2026-11-22**: MEDIA502 bajó de 5,1 MiB a 20 KB ([STATUS](docs/STATUS.md#identidad-portable-f0--2026-11-22)). Lo que se decidió (decisión del usuario, 2026-11-22; [decisión abierta 4 de `nu9ve-volume-v1` §13](vendor/nu9ve-core/contracts/nu9ve-volume-v1.md#13-decisiones-abiertas)). Hoy `index.json` lleva un dataset por archivo: MEDIA502 tiene 7 666 de unos 698 B, 5,1 MiB de 8, con el disco al 57 %. Sin cambios, el tope llega hacia las 12 000 entradas y el disco lleno serían unas 13 400. Con otra ráfaga como la del 8 nov (3 622) se pasa. El cambio:
   - un dataset por **proyecto × carpeta de primer nivel** (`001_raw/…`, `005_produccion/…`) y modalidad, con `size.items` y `size.bytes` sumados, `updated_at` el más reciente y `state` el peor de sus archivos. Lo que no tiene proyecto va a un dataset por carpeta;
   - el detalle por archivo (hash, ruta, verificación) sigue en el snapshot del catálogo de `.ncreator/catalog-backups/`, que la adopción ya comprueba por SHA-256;
   - no cambia el contrato: `nu9ve-data-v1` no dice que un dataset sea un archivo. Lo que sí sube es el `contract` interno de nüün en el índice, para que un lector viejo lo reconozca;
   - medir antes y después: MEDIA502 debería bajar a decenas de KB y crecer con los proyectos, no con los archivos;
   - la adopción sigue leyendo conteos y proyectos igual; «Adoptar este disco…» muestra datasets y archivos por separado.
2. **Otra Mac y Windows**: los [pasos exactos](docs/STATUS.md#pendientes-físicos-de-f0) los corre el usuario.
3. Pulir la ventana de adopción: decir qué hará F1 con lo adoptado. La fecha local ya está.

### F1 · Agente, registro, `data_catalog` y leases

**Hecho el 2026-11-23** ([STATUS](docs/STATUS.md#suite-f1-con-el-agente--2026-11-23)): core en `1d11d30`, agente en
Ajustes (`nu9ve agent state` e `install --by nuun`), `device.json` del agente, socket registrable con `data_catalog`
y `data_export`, colecciones por dispositivo y «Trabajar aquí». Lo que sigue, en orden:

1. ~~Aceptación en ventana e instalar~~ **Hecho el 2026-11-23** (`79eb92ddcfa6`). Falta abrirla en uso diario.
2. Claves de suite legibles para los proyectos: hoy la clave es el id local (un UUID), y `baches` en nove no
   coincidiría con nüün ([data §2](vendor/nu9ve-core/contracts/nu9ve-data-v1.md#2-clave-de-proyecto-común)).
3. F0 en la M4 ([pasos](docs/STATUS.md#pendientes-físicos-de-f0)), y después emparejar M1 y M4.
4. La colección Agentes (`~/.claude`, `~/.codex`) en `data_catalog` como `files`/`personal`, y una colección viva
   por dispositivo para respaldarla en MEDIA (la clave conocida `conversaciones` la acepta la persona).
5. Windows: named pipe con las reglas de motor §9 para registrarse.
6. Leer el estado de lease al abrir un proyecto y dejarlo en sólo lectura cuando otra máquina es la dueña.

### Recorrido de misiones entre la M1 y la M4 (pedido del usuario, 2026-11-23)

Guardar las misiones en la M1 (MEDIA + ESSD), llevar el ESSD a la M4, copiar al SUBSSD, procesar ahí con Atlas, devolver lo procesado al ESSD y analizarlo en nove en la M1. MEDIA se requiere para el ingreso de material nuevo. La copia entre discos de trabajo puede hacerse en la M4 mientras MEDIA sigue en la M1. [Flujos](docs/features/suite-disks-flows.md).

1. **Ya hay:** el ingreso de dron deja la misión en MEDIA y en el ESSD. `work_copy` copia verificado entre dos discos de trabajo de la misma Mac, con recibo en los dos. Fuera del sandbox no hay otra guarda que el marcador y el rol. Nunca se corrió con discos reales.
2. **Primera ida física hecha.** ESSD → SUBSSD completada y verificada en la M4, incluidos el historial y el mapa de fotos ([evidencia](docs/STATUS.md#pendientes-físicos-de-f0)). La hoja de copia ya incluye juntos los archivos de Atlas en el código. Pendiente validar esa selección en ventana y hacer la vuelta de resultados a la M1.
3. ~~**Copiar proyecto**~~ **Hecho el 2026-11-23** ([STATUS](docs/STATUS.md#discos-de-trabajo-y-conversaciones--2026-11-23), punto 7): la hoja lista los proyectos de nüün del índice publicado en el disco, así funciona en la M4 sin el catálogo de la M1. Antes de llevar el ESSD, «Publicar índice» en Ajustes para que incluya las misiones recientes. Falta un proyecto de Clipper con sus dependencias (`clipper.inspect`).
4. ~~**Volver con lo procesado**~~ **Hecho el 2026-11-23**: «Actualizar lo que cambió» (la versión anterior queda en la carpeta de nüün del destino) y la sugerencia `work_copy_changed` desde los recibos, sin necesitar un índice de Atlas. Sólo ve lo que cambia dentro de las carpetas copiadas: si Atlas escribe en otra carpeta, se elige a mano.
5. **nove registra desde el recibo** (sesión de nove, A70–A76): sin eso, nove vuelve a hashear lo que llega al ESSD.
6. Opcional: emparejar M1 y M4 para «Trabajar aquí» (F1, paso 3).

### F1 · Diarios de decisiones ([§6.4](vendor/nu9ve-core/docs/suite-devices.md#nüün-en-dos-macs))

Cada Mac escribe lo que decide y nüün junta los diarios de todas; el catálogo SQLite pasa a ser una caché que se reconstruye. Cómo serían:

- **Dónde:** `devices/<device_id>/nuun/decisions.ndjson` en la carpeta de intercambio (`nu9ve-exchange-v1`, todavía sin contrato). Lo escribe sólo la Mac dueña de ese `device_id`, sólo añadiendo líneas; las demás lo leen.
- **Una línea por decisión:** `{"seq":N,"device_id":…,"at":"…Z","kind":…,"key":…,"field":…,"value":…,"prev":<seq o null>}`. `seq` es el contador de esa Mac, sin huecos, y `prev` la última decisión que esa Mac vio sobre la misma clave y campo. Con eso se distingue «cambió después de ver lo otro» de «cambiaron a la vez».
- **Qué entra:** crear un proyecto (clave común de [data §2](vendor/nu9ve-core/contracts/nu9ve-data-v1.md#2-clave-de-proyecto-común)), renombrarlo, asignar o quitar un asset, cerrar, publicar y los recibos nuevos por su ruta en el disco. **No entra** nada que libere o borre: eso se sigue decidiendo con los recibos verificados de los discos conectados. Un diario atrasado nunca autoriza borrar.
- **Unir:** lo que se suma (asignar, añadir un recibo, crear con una clave nueva) se une sin preguntar. Dos valores para el mismo campo cuyos `prev` no se ven entre sí son un conflicto: nüün lo muestra con las dos Macs y sus fechas, y la persona elige. La elección es otra decisión en el diario de quien eligió.
- **Reconstruir:** el catálogo local se puede rehacer con los índices de los discos más los diarios. Se prueba borrando el catálogo de QA y comparando el resultado con el original.
- **Migrar:** el catálogo actual de la M1 se publica una vez como decisiones con `seq` desde 1, marcadas como importadas; la M4 empieza vacía y adopta.
- **Antes de construirlo:** que el core cierre `nu9ve-exchange-v1` y el transporte (Syncthing de andamio), y que la [pregunta 9](vendor/nu9ve-core/docs/suite-devices.md#12-preguntas-para-el-usuario) (quién corre el agente) esté decidida.

## Auditoría del 2026-11-10 (propuestas, sin priorizar aquí)

La [auditoría Fable](vendor/nu9ve-core/docs/audits/fable-audit-2026-11-10.md) revisó nüün, nove, curado y Fractos en el PC de Windows y propone hallazgos con [briefs por sesión](vendor/nu9ve-core/docs/audits/session-briefs-2026-11-10.md). Un hallazgo entra a una tabla de arriba cuando el usuario lo elige. Ya entraron (2026-11-11) a la [ruta del motor](#motor-en-rust--ruta): verificación sin caché ([A1](vendor/nu9ve-core/docs/audits/fable-audit-2026-11-10.md#a1--la-relectura-de-verificación-no-evita-la-caché-fuera-de-macos), paso 1), dorado en git ([B2](vendor/nu9ve-core/docs/audits/fable-audit-2026-11-10.md#b2--no-hay-ci-y-el-diferencial-nunca-ha-corrido), paso 2) y backend de macOS ([B6](vendor/nu9ve-core/docs/audits/fable-audit-2026-11-10.md#b6--el-motor-decide-por-cfgwindows), paso 6). De [S3](vendor/nu9ve-core/docs/audits/session-briefs-2026-11-10.md#s3--higiene-de-repos), en este repo quedaron hechos el 2026-11-11 `.gitattributes`, versiones fijadas, README y `windows.md` §1; los enlaces a documentos compartidos de este repo ya apuntan a la copia fijada.

Decidido por el usuario el 2026-11-10: sonido de interfaz **encendido por defecto, con una opción clara para silenciar**, en toda la suite ([sonido v1 §7](vendor/nu9ve-core/contracts/nu9ve-sound-v1.md#7-valor-por-defecto-ajustes-y-formato)).

Decidido el 2026-11-11: lo compartido de la suite vivirá en un repo `nu9ve/core` y cada app llevará una **copia fijada** ([cómo](vendor/nu9ve-core/docs/suite-sharing.md)). nüün ya fija los contratos, datos, documentos, los crates `nu9ve-json`, `nu9ve-fs` y `nu9ve-engine`, y `swift/` para `Nu9veTheme` del remoto privado `nu9ve/core`. La adopción del tema fijó el mínimo de la app en macOS 15. Ver [olas de adopción](vendor/nu9ve-core/docs/suite-adoption.md) e [inventario de la mudanza](vendor/nu9ve-core/docs/suite-split.md). Al cerrar la semana de uso diario, cambiar `ShellWindow`, `SkinChrome` y `AboutView` por `NuMacShell` en un cambio de interfaz propio. C# espera Windows.

La §12.1 de `nu9ve-engine-v1` está implementada en los dos motores y Rust usa el servidor de líneas compartido. [Evidencia del gate y de la sombra sobre el dorado](docs/STATUS.md#nu9ve-engine-v1-en-los-dos-motores--2026-11-16).

Decisiones que esperan: canal de firma y actualización, cuarentena o papelera al liberar. Para nüün en Mac queda propuesto el estándar de [avisos del sistema](vendor/nu9ve-core/contracts/nu9ve-notifications-v1.md): hoy sólo avisa de una tarjeta detectada.

## Pasada de UX · discos y conversaciones

- Unificar el alta de disco y guiar el uso local de un disco portable detectado. Mostrar por qué aparece una actualización, con proyecto, dirección y cambios. [Criterios de UX](docs/features/suite-disks-flows.md#pasada-de-ux-pendiente).
- Validar en ventana el export a pedido sin MEDIA y en nove el árbol de M4 y M1 en ESSD. Comprobar el progreso compartido en una copia larga. [Recorrido](docs/features/suite-disks-flows.md#reunir-los-historiales-de-m4-y-m1-en-essd).

## Prioridad baja · capturas del producto

Decidido por el usuario el 2026-11-23: no se actualizan las capturas hasta la revisión completa de UX y UI, cuando las pantallas ya queden como van a ser. Entonces se sigue el [flujo de capturas](docs/development.md#capturas-del-producto). Las pantallas nuevas desde las últimas son Inicio → «Discos conectados», la hoja «Copiar al otro disco…» y Ajustes → Conversaciones.

## Prioridad baja · nombre interno

Renombrar `MediaMenu` → `Nuun` (paquete, targets, ejecutable, prefijo `Nu9veMedia-local-*`, scripts y docs) en un cambio propio sin comportamiento nuevo. Se conservan `CFBundleIdentifier` `app.nu9ve.media`, `~/Applications/nuun.app` y el catálogo.

## Retomar en otra sesión

Leer CONTEXT → STATUS (tabla por área) → fila de este PLAN → contrato o módulo dueño. Registrar sólo resultados observados con job/build; no heredar aprobación física de otra build o de fixtures. Respetar `git status`, sin commits/restage ni mensajes a otras sesiones como efecto lateral. [Formato de handoff](docs/development.md#cerrar-y-retomar-una-sesión).
