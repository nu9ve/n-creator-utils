# Motor de nüün en Rust

Adoptado el 2026-11-09 ([análisis y ruta](../docs/platforms/rust-engine.md), [PLAN](../PLAN.md)). Habla el mismo protocolo que `python -S nu9ve.py media rpc-serve`: una línea JSON por petición `{id, method, params}` y una por respuesta, `{id, ok: true, data}` o `{id, ok: false, error: {code, message}}`. Python (`media/`) es el **oráculo** hasta que cada método llegue a paridad. Un método de lectura que todavía no está portado responde `not_ported`, y el cliente pregunta a otro motor.

| Crate | Qué tiene | Origen en Python |
| --- | --- | --- |
| `crates/nu9ve-json` | JSON canónico **byte a byte** igual al de Python (claves ordenadas, sin espacios, escapes de Python, `repr` de los decimales: `1e-07`, `1e+16`, `1.0`), digest SHA-256, error `{code, message}`, sobres de `rpc-serve` | `media/common.py` `canonical`, `digest`, `MediaError`; `media/cli.py` `serve_reads` |
| `crates/nu9ve-volumes` | Identidad de volumen en Windows (GUID, serial, discos físicos, bus, medio extraíble, índice de la raíz, espacio); letras montadas, sin administrador. Módulo `fs` para quien mueve media: sync completo (`F_FULLFSYNC`), sync de carpeta, renombrado que nunca reemplaza, apertura sin seguir enlaces, rutas largas `\\?\`, y `verify_read`: la relectura que prueba una copia sale del dispositivo, no de la caché (`F_NOCACHE` en Mac, `FILE_FLAG_NO_BUFFERING` por `ReOpenFile` con bloques alineados en Windows, `posix_fadvise(DONTNEED)` en Linux; si el sistema de archivos lo rechaza, lo dice con `Cache::Os`) | curado `identity.rs`/`relink.rs` (Windows); `media/volumes.py` (Mac, sigue en Python) |
| `crates/nu9ve-time` | Tiempo civil con la semántica de `datetime` de Python: `fromisoformat`, `isoformat`, `strftime` (C locale), `fromtimestamp` con redondeo a par y `astimezone()` con el algoritmo de CPython (`local_to_seconds`: pliegues y huecos de horario de verano). En Mac la hora local sale de `localtime_r`, la misma de Python, y respeta `TZ`; en Windows, de la zona del sistema (`chrono`) | `datetime`, `media/evidence.py` |
| `crates/nuun-recipe` | La receta como datos (`template.json`, transcripción de `TEMPLATE`), receta efectiva, familias de tarjeta por las dos primeras carpetas (contenido primero), resumen de `recipe` | `media/recipes.py`, `media/volumes.py` `card_hint` |
| `crates/nuun-media` | Catálogo SQLite con lecturas que no crean el catálogo y escrituras con el esquema de Python. Comparte el lock de escritor (`writer.lock`, el mismo `flock` que Python en Mac). Reglas de `slug` y rutas relativas, identidad, independencia física, destinos, `volumes`, `source_cards` y `card_hint`; escaneo de tarjetas, EXIF/XMP de fotos y dispositivo por receta; misiones (índice y match), nombres de destino (`layout`), lecturas seguras bajo una raíz (`tree`) y el planner de tarjetas | `media/catalog.py`, `media/volumes.py`, `media/service.py`, `media/planner.py`, `media/photo_meta.py`, `media/recipes.py`, `media/missions.py`, `media/layout.py`, `media/common.py` `Tree` |
| `nuun-engine` | Binario `nuun-engine [--state-dir DIR] rpc-serve \| rpc \| --version` | `media/cli.py` |

Los `nu9ve-*` son candidatos a compartirse con nove, curado y kyu ([§6 del análisis](../docs/platforms/rust-engine.md#6-qué-compartir-en-rust-entre-las-apps), [qué se muda y cuándo](../docs/suite-split.md)); los `nuun-*` son de nüün. Ninguna pieza `nu9ve-*` nombra algo de nüün: lo comprueba `scripts/check_suite_boundary.sh`.

**Backend de volúmenes por plataforma.** `nuun_media::volumes::platform_backend(home)` es el único lugar que pregunta por el sistema operativo: en Windows da `WindowsBackend`; en **macOS, `MacBackend`** (desde el 2026-11-12); en Linux, `None` todavía. El binario no tiene `cfg(windows)`: los métodos de volúmenes usan ese backend o responden `not_ported`, y `engine_info` dice `volume_backend`. En macOS, `Backend::writes` habilita las escrituras de volumen cuando el catálogo efectivo pertenece al sandbox. En uso diario, la app usa Python para las escrituras hasta la aceptación con hardware. [Gates](../docs/release.md).

**`MacBackend`** ([brief S16](../docs/audits/session-briefs-2026-11-10.md#s16--backend-de-volúmenes-de-macos-en-rust)) copia paso a paso `VolumeBackend.describe` de Python y lee **las mismas fuentes**, para que un destino registrado por Python quede en línea en Rust sin registrarlo de nuevo: `statfs` (lo que imprime `df -P`), `diskutil info -plist` (`VolumeUUID`, `DiskUUID`, almacenes físicos de APFS, `Internal`, medio extraíble, imagen de disco), `statvfs` (espacio, como `shutil.disk_usage`), y `st_dev` con signo de 32 bits, como lo da Python. `diskutil list -plist external` da los montajes. Se eligió `diskutil` y no DiskArbitration directo porque la paridad de identidad era la condición; la ganancia viene de describir los discos **en paralelo** (`Cached::prefetch`), ya que Python los describe uno tras otro. DiskArbitration queda como optimización posterior, medida con la sombra. Las piezas genéricas (`statfs`, `statvfs`, `diskutil`, el lector de plist) viven en `nu9ve-volumes::macos`.

## Métodos

| Método | Estado | Prueba |
| --- | --- | --- |
| `recipe` | **Portado**: la receta guardada en el catálogo (o la plantilla) y los dispositivos confirmados, como Python | Unitarias, contrato desde C# y diferencial con Python |
| `volumes` | **Portado en Windows y macOS**: cada destino registrado con `online` (identidad comparada por `mode`, `volume_uuid`, `disk_uuid`, `root_inode`). | Pruebas portadas de `test_media_engine`/`test_media_eject` con un backend de fixture; el volumen del sistema registrado como destino con el backend real; contrato desde C# |
| `source_cards` | **Portado en Windows y macOS**: unidades extraíbles y externas, nunca un destino ni un volumen en el mismo disco físico, ni imágenes. `hint` (familia, recomendación, uso, último proyecto) **sólo** con un destino en línea. `hint.device` del EXIF de la primera foto bajo DCIM (`card_device`, a lo más 64 entradas), que cambia la recomendación de un dispositivo conocido | Igual que `volumes`, más el diferencial de familias sobre árboles generados por Python |
| `snapshot` | **Portado**: `volumes` (con `online` y `free`), `library`, `jobs` (con la etapa Sony), `profiles`, `projects`, `release_plans`, `integration_settings`, `defaults`, `project_closures` y `source_cards`. Consulta en paralelo `capabilities` de Clipper y Stalker. Incluye el retiro registrado cuando falla la lectura final de una tarjeta Sony. | Unitarias, contrato desde C# y diferencial con Python |
| `discover` | **Portado para tarjetas**, sólo por `rpc` (como Python). Recorrido propio sin seguir enlaces ni puntos de reanálisis (el de Python usa `O_NOFOLLOW`/`O_DIRECTORY` y no corre en Windows), archivos ignorados, residuos de Finder, validación de bundles Stalker, evidencia de cámara (incluido el XML de Sony), agrupado en unidades. Lee EXIF/XMP de JPG y DNG (`photo_meta`, sin dependencias) y da `device` (registro confirmado, regla de la receta, evidencia de vuelo o «por confirmar») y `created_at` (hora de cámara). Con `archive`, `mission` contra el registro de misiones del archivo (mismo nombre, hora de cámara y posición; sin archivo en línea, nada, como Python); donde no hay backend de volúmenes (Linux hoy) lo dice en `not_ported`. Igual que Python tras corregir allí un error (ver abajo). Una fuente con sesiones Stalker responde `not_ported` (sus metadatos siguen en `captures.py`). Colisiones de nombre sin NFC (sin crate de normalización); un caso no detectado igual falla seguro al renombrar | Unitarias de escaneo, EXIF (JPEG y TIFF sintéticos, archivos dañados, redondeo de Python) y dispositivo; contrato desde C# con un JPEG con EXIF; diferencial sobre árboles de tarjeta generados por Python |
| `jobs`, `job`, `library`, `projects`, `profiles`, `devices`, `release_plans`, `defaults`, `integration_settings` | **Portados**: las mismas lecturas que `snapshot` junta, como métodos propios. `library` filtra por proyecto (`casefold` para títulos: minúsculas, `ß`→`ss`) y estado de trabajo; `job` da la fila completa, el plan y el progreso. `jobs` reconstruye la etapa Sony desde los recibos y el diario; `defaults` e `integration_settings` sólo sin parámetros (con parámetros son escrituras) | Unitarias con el esquema completo de Python |
| `engine_info` | Nuevo, sólo Rust: qué motor respondió, qué métodos tiene en esta plataforma (`ported`, `writes`) y qué backend de volúmenes (`volume_backend`: `"windows"` o `null`) | Unitaria + contrato |
| `project_preview`, `closure_overview`, `closure_link_candidates`, `closure_progress`, `action_progress`, `asset_revisions`, `living_collections`, `missions_status` | **Portados** (fase L, 12 nov), en todas las plataformas: lecturas del catálogo (`reads.rs`; `closure_overview` es el `project_closures` del `snapshot`). `project_preview` no normaliza a NFC como Python: un título descompuesto daría otro nombre | Dorado de lecturas: Python arma un catálogo de prueba, guarda su SQL y sus respuestas a 29 peticiones; Rust carga el mismo SQL y responde por su despacho de `rpc-serve` (`catalog_reads_match_python`) |
| `integration_capabilities` (y `capabilities` del `snapshot`) | **Portado** (12 nov): busca Clipper y Stalker como Python y los consulta con tope de 3 s, **los dos a la vez y en paralelo con las lecturas del `snapshot`** (`integrations.rs`). No anota `clipper-known` (el catálogo es de sólo lectura; lo sigue anotando Python), pero responde `known` como Python justo después | Unitarias con ejecutables falsos (éxito, fallos, ausencia, tope de tiempo); sombra sobre la Mac real con Clipper y Stalker instalados: igual |
| `capture_libraries` | **Portado** (12 nov): bibliotecas de Stalker (registro del productor, override, carpetas habituales con sesiones), manifiestos, vista previa, archivadas y etiquetas (`captures.rs`). Usa el índice `capture-index:` que guarda Python sin reescribirlo | Dorado: 9 carpetas de sesión (completas, en grabación, rotas, de otro productor, archivada, sin manifiesto, fecha inválida, en un registro con rutas faltantes) recreadas por Rust (`libraries_match_python`) |
| `capture_overview`, `closure_deliveries`, `suggest` | **Portados** (12 nov) en plataformas con backend de volúmenes (`projects.rs`, `volumes::suggest`): vista de proyectos, estado de cada entrega final (archivada, por publicar, publicada, movida al publicado, respaldada por recibo, sin rastro, disco desconectado) y la sugerencia de ingreso | Dorado de proyectos: disco de trabajo, archivo, disco registrado que ya no está, recibos, alias, sesiones y estados de job (`deliveries_and_overview_match_python`); unitaria de una entrega movida a mano; sombra real |
| `volumes`, `source_cards` en macOS | **Portados** con `MacBackend` (12 nov) | Unitarias del plist y del volumen del sistema; sombra sobre MEDIA502 y ESSD montados: iguales, incluido el espacio libre |
| `closure_candidates` | **Portado** (12 nov, plataformas con backend): finales del perfil en cada disco en línea que corresponden al proyecto (título, alias, búsqueda), con lo ya decidido, el rol sugerido y si la publicación cambió (`projects.rs`, con las expresiones de `closure.py` sobre texto en minúsculas) | Dorado de proyectos: 4 búsquedas (sugeridos, búsqueda explícita, otro proyecto del grupo, búsqueda vacía); sombra real |
| `storage_overview`, `storage_progress`, `storage_tree` | **Portados** (12 nov; `overview` con backend): leen el índice de discos (`storage-index.sqlite3`) **sólo para leer**. Si falta o es de otra versión, responden como un índice vacío; Python lo crearía vacío. La clasificación de cada archivo es la de `_Evidence`: verificado, probable, sólo aquí, derivado o desconocido (`storage.rs`). Escanear y comparar siguen en Python | Dorado de almacenamiento: Python escanea un archivo y un disco de trabajo con recibos y tamaños repetidos, y Rust lee el mismo índice (11 consultas); sombra sobre el índice real de 205 mil archivos |
| `missions_review` | **Portado** (12 nov; con `archive`, plataformas con backend): valida cada exportación, simula el merge del registro de misiones sobre una copia en memoria y calcula la cobertura y el digest (`mission_review.rs`). `incorporate` sigue en Python. Diferencia conocida: el texto de `error` de un archivo ilegible o con JSON inválido | Dorado de misiones: bandeja con exportaciones nuevas, viejas, con borrados, inválidas y ya incorporadas, contra un registro en el archivo (5 casos); sombra real sobre `~/Downloads` |
| `finals_status`, `captures`, `publications`, `retitle_operations`, `collection_release_plans`, `resolve` | **Portados** (12 nov) como lecturas de un solo uso: sólo por `rpc`, como en Python (`engine_info.once`). `captures` acepta `changed_paths` y no reescribe el índice `capture-index:` (Python sí lo hace) | Dorado de lecturas, de proyectos (`finals_status` con sugerencia de cierre) y de capturas (`changed_paths`); repetición real en modo de un solo uso |
| `sources`, `capture_discard_check` (y `clipper.inspect`) | **Portados** (12 nov) como lecturas de un solo uso. `sources`: las fuentes de un proyecto para un editor, con la copia preferida, alternativas, copias que faltan y metadatos de recibo (`sources.rs`). `clipper.rs`: el reporte de Clipper, proyectos antiguos que no reporta, registro de proyectos vinculados y cola de exportación, como Python; lo usarán también liberar y cerrar | Dorado de proyectos (4 consultas de `sources`) y de Clipper (Clipper, `HOME` y carpeta de proyectos de prueba: 4 `inspect` y 5 `capture_discard_check`); repetición real de `sources` |
| E1a · escrituras del catálogo: `defaults`/`integration_settings` con parámetros, `dismiss_job`, `touch_project`, `set_project_state`, `rename_project`, `set_work_state`, `set_publication`, `save_capture_tag(s)`, `set_finals_folder`, `cancel_plan`, `capture_discarded`, `closure_link`, `save_profile`, `confirm_device` | **Portados** (12 nov), en todas las plataformas, sólo por `rpc` (`CATALOG_WRITES`, en `engine_info.writes`). Nunca tocan media; toman el lock de escritura donde Python lo toma y no en otro lado (`settings.rs`). La app las usa con Rust en el sandbox | Dorado de escrituras: el mismo catálogo base y 45 escrituras (válidas y rechazadas); las mismas respuestas y el mismo catálogo final, tabla por tabla, con las horas normalizadas (`catalog_writes_match_python`) |
| `closure_save` | Guarda roles requeridos, decisiones de publicación y stamps de finales, bajo el lock del catálogo. Guarda la configuración y el evento juntos | Dorado de decisiones y rechazo. Comparación con un final en HFS+ de prueba y rechazo de duplicados |
| `backup_metadata` | RPC independiente en Windows y el sandbox de macOS. Reutiliza el respaldo que corre después de un ingreso | Comparación en sandbox del recibo, hash e integridad SQLite |
| `coverage`, `storage_add_folder`, `storage_remove_folder` | Informe SHA-256 o por tamaños y gestión de carpetas observadas bajo `storage.lock`. El índice escanea y compara con Python | Diferencial de respuestas y del informe, rechazo de bloqueo ocupado y preservación de archivos |
| `plan_release`, `review_release`, `plan_finals`, `plan_finals_document` | Planes revisados que comparten el catálogo con Python. El cierre de proyecto conserva su ruta Python | Diferencial de liberación y finales; revisión rechaza digest distinto |
| `apply_release`, `relink_release` | En el sandbox de Mac, usan el mismo flock y token que Python, verifican copias y dependencias, y comparten el diario de retiro y de ACK | CLI Clipper de prueba, corrupción, interrupción tras unlink y reintento |
| `retitle_capture`, `publish_revision` | Stalker aplica el cambio de proyecto bajo ambos bloqueos heredados. La publicación verifica una copia separada y conserva la revisión anterior | Fixture del productor compartido con Python. Muerte del coordinador, reintento, invalidación de planes y recuperación desde recibos |
| `relayout` | Renombra las copias verificadas a la ruta fechada, con dependencias de Clipper y diario de renombre | Interrupción tras renombrar una copia, actualización de recibos y repetición |
| `transcripts_backup` | Respaldo a pedido de transcripciones exportadas por Clipper. Vincula por identidad del miembro o ruta de trabajo, verifica SHA-256, conserva versiones y guarda el recibo | Diferencial con Python de primera copia, nueva versión y adopción; fixture CLI de Clipper y detección de corrupción o cambios |
| `closure_plan`, `closure_apply` | En el sandbox de Mac, cubre finales publicados y cierre editorial completo, con fuentes, intermedios, transcripciones y regenerables gestionados por Clipper | Diferenciales de ambos modos. Fixtures de reintento, proyectos vinculados, cambios de inventario y archivos alterados |
| `handoff` | Lectura de un solo uso. Produce `creator-media-handoff/1` a partir de recibos y comprueba la consistencia entre réplicas | Siete casos diferenciales de selección, miembros, volúmenes y conflicto |
| `eject_card`, `confirm_camera_cleanup`, `reconcile_sony_cleanup` | Expulsión normal de macOS bajo el lock de ingreso y seguimiento de limpieza por identidad, recibos y estado de los índices | Fixtures de identidad, bloqueo, archivos reaparecidos y formato. Falta aceptación física |
| `deliver_work`, `restore`, `recover_catalog` | Copias verificadas, entrega con recibo y progreso, y reconstrucción desde recibos del archivo | Origen ausente, corrupción del mismo tamaño, destino desconectado, reintento y conflicto |
| Variantes de lectura | `discover` de fuentes Stalker sigue en Python | `test_engine_boundary` mantiene la lista de lecturas persistentes alineada con Python |
| `register_volume` | **Portado en Windows y el sandbox de macOS**, sólo por `rpc`. Configuración, nunca media: describe el volumen (externo salvo `allow_local`), toma el lock, valida rol, nombre, prefijo y duplicados, y guarda el registro canónico con un UUID v4 | Pruebas portadas de los rechazos de Python; lock ocupado → `job_busy`; esquema y WAL del catálogo creado; contrato desde C#; registro desde la ventana por UI Automation |
| `plan`, `plan_document` | **Portados para tarjetas en Windows y el sandbox de macOS**, sólo por `rpc`. `_build_plan` paso a paso y en el orden de Python (el primer chequeo que falla decide el código): política de sólo-archivo, identidad del origen, discos físicos distintos, solapes, escaneo, asignaciones (proyecto, perfil, país, cámara, trabajo, descarte), dispositivo por EXIF, misión contra el archivo, copias ya presentes en la raíz del consumidor (`WorkPresence`), fotos ya respaldadas, `layout` (nombres de Python: hora de cámara, XML de Sony, WAV, ffprobe si está, mtime en la zona del sistema), ids `uuid5` y `digest`, vista previa, conflictos de destino, `validate_plan` y `preflight` con espacio. Guarda `reviewed-plan:<id>` bajo el lock. Incluye Stalker, entregas y finales. Un manifiesto con zona IANA explícita sigue en Python | Pruebas portadas de `test_devices` y `test_mission_match` (nombres de dron, misión emparejada, DJI que pregunta, par de dos dispositivos, códigos de política, descartes, solape, mismo disco, lock ocupado, plan cambiado); en el PC, `plan` real por el binario con una carpeta local; diferencial de 7 tarjetas contra Python y comparación con imágenes montadas |
| `apply`, `resume` | **Portados para tarjetas en Windows y el sandbox de macOS**, sólo por `rpc`, bajo el lock de escritor. `Transfer.apply` de Python paso a paso: cada copia va a `.<nombre>.ncpartial` (una lectura del origen escrita en paralelo a cada destino), se sincroniza con su carpeta (`F_FULLFSYNC` en Mac), se relee y se compara por SHA-256 antes de renombrarla **sin reemplazar nunca** (`MoveFileExW` sin reemplazo; `renamex_np` exclusivo en Mac). Un destino con los mismos bytes se adopta; uno distinto es `destination_conflict`. Diario en `members`/`copies` por archivo: un job interrumpido se reanuda y toma una réplica verificada en vez de volver a leer la tarjeta. Las relecturas que verifican (cada parcial, un destino adoptado, el original antes de limpiar) van por `fs::verify_read`; si alguna no pudo evitar la caché, el recibo y el resultado llevan `verify_cache: "os"` o `"dropped"` (cuando la evitó, no llevan nada, como Python). Recibos en `.ncreator/receipts/`, índice (`assets`, `work`, proyectos), limpieza sólo tras verificar todo, reescanear y **releer cada original** con el mismo hash (los índices Sony nunca se borran), descartes al final, `source_has_new_material` si apareció algo. Después, `units.ndjson` de misiones y respaldo del catálogo en el archivo (`VACUUM INTO`, verificado). Copia también bundles Stalker y entregas que retienen el origen. La liberación automática de Stalker y la limpieza de finales usan la exclusión de Clipper en Unix. Rutas de más de 260 caracteres en Windows (forma `\\?\`) | Pruebas portadas de `test_media_engine` (dos destinos y limpieza, reintento idempotente, corrupción del mismo tamaño, copia interrumpida, caída antes de renombrar, limpieza parcial reanudada, archivo nuevo antes y durante la limpieza, destino ajeno, destino idéntico adoptado, origen cambiado, desconexión antes y durante la limpieza, corrupción durante la limpieza, Sony conserva índices, descartes, plan cambiado o cancelado, lock ocupado, `resume` y respaldo del catálogo) y de `test_mission_match` (misión → carpetas y `units.ndjson`); en el PC, `plan` → `apply` → `resume` por el binario; el dorado aplica cada plan y compara estado y copias verificadas con Python |
| Otras mutaciones | `read_only` por `rpc-serve`; `not_ported` por `rpc` (`confirm_camera_cleanup`, `reconcile_sony_cleanup`, `eject_card`, `recover_catalog`, releases, finales…) | Unitaria |

**Error de Python encontrado al portar y corregido** (`media/service.py`, `discover`, 2026-11-09): `metadata_device` sólo se asignaba si había dispositivo y se borraba si no había hora de cámara. Una foto con dispositivo y sin `DateTimeOriginal` perdía su `device`; una foto fechada sin `Make` heredaba el `device` de la unidad anterior, o lanzaba `UnboundLocalError` si era la primera. Ahora cada unidad empieza vacía, en Python y en Rust. Prueba nueva en `tests/test_devices.py` (`test_discover_keeps_each_units_device_and_camera_time_apart`), **sin ejecutar**: no hay Python en este PC.

**Dorado en la Mac** (lo que valida todo lo de arriba contra Python; verde desde el 2026-11-11): `python3 scripts/engine_golden.py --check`, luego `cd engine && TZ=America/Hermosillo TMPDIR="$(cd "$TMPDIR" && pwd -P)" cargo test --offline -- --ignored`. `TZ` porque los nombres sin hora de cámara usan la zona del sistema; `TMPDIR` resuelto porque las carpetas temporales de macOS cuelgan de `/var`, un enlace que el escaneo rechaza. En la Mac, `cargo` necesita descargar una vez `libc` si no está en su caché.

**Cambio de identidad en Windows (2026-11-10):** `root_inode` es ahora el índice de la **carpeta registrada**, como Python usa el `stat` de la raíz que recibe (antes era el de la raíz de la unidad). Un destino registrado antes en Windows aparece fuera de línea (`volume_replaced`) y se registra de nuevo; sólo había registros de QA.

La secuencia vigente está en [PLAN](../PLAN.md#motor-en-rust--ruta). Falta validar Ingreso en la ventana y con hardware. Un corte del proceso Rust puede dejar el job en el último estado registrado; el diario permite retomarlo con `resume`. Python ya abrió catálogos escritos por Rust en el sandbox de Mac.

### Identidad de volumen en Windows (diseño nuevo)

Python no tiene backend de Windows: fuera de macOS usa un modo `local` sin identidad física y no lista montajes. El modo `windows` cumple las mismas garantías que el de Mac:

| Campo | De dónde sale | En Mac |
| --- | --- | --- |
| `volume_uuid` | Ruta GUID del volumen (`\?\Volume{…}\`): no cambia con la letra | `VolumeUUID` |
| `disk_uuid` | Serial del sistema de archivos (8 hex) | `DiskUUID` |
| `physical_disks`, `physical` | `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` → `PhysicalDriveN`; sin discos, el volumen no se describe (falla cerrado) | `APFSPhysicalStores` o disco padre |
| `external`, `removable`, `disk_image`, `bus` | `IOCTL_STORAGE_QUERY_PROPERTY` (bus USB/SD/MMC/1394, medio extraíble, VHD/ISO) y `GetDriveTypeW` | `Internal`, `RemovableMedia`, `Disk Image` |
| `root_inode` | Índice de archivo de la raíz (`GetFileInformationByHandle`) | `st_ino` |
| `mount`, `label`, `filesystem`, `block_size`, `free` | Letra en mayúscula, etiqueta, sistema de archivos, unidad de asignación, espacio para el usuario | lo mismo |

Límites: la ruta GUID es propia de cada PC (un catálogo de Windows no reconoce sus discos en otro equipo; en Mac `VolumeUUID` viaja con el disco). Un volumen dinámico o un RAID de software aparece con todos sus `PhysicalDriveN`: no es independiente de ningún volumen que comparta alguno. No se probó con una SD ni un disco USB reales: las pruebas usan fixtures y el volumen del sistema.

## Compilar y probar

```powershell
cd engine
cargo build --offline
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo fmt --check
bash ../scripts/check_suite_boundary.sh
```

El compilador está fijado en `rust-toolchain.toml` (1.99.1, con `rustfmt` y `clippy`); en una máquina sin esa versión, `rustup` la instala una vez. Estado del 2026-11-11: en Windows 11, 108 pruebas verdes y 5 ignoradas (las diferenciales), fmt y clippy limpios; en **macOS**, compilado sin descargas (el `cargo` de Homebrew ya es 1.99.1), 110 pruebas verdes con las 5 diferenciales, fmt y clippy limpios. En macOS las pruebas necesitan `TMPDIR` resuelto (las carpetas temporales cuelgan de `/var`, un enlace que el motor rechaza, como Python): `export TMPDIR="$(cd "$TMPDIR" && pwd -P)"`. La CI lo hace igual ([checks.yml](../.github/workflows/checks.yml)).

Las dependencias son las mismas versiones que nove y curado y ya estaban en la caché local: `serde`, `serde_json`, `sha2`, `regex` (las condiciones `dcim_regex` son datos editables), `windows-sys` y `tempfile` para las pruebas. Ninguna se descargó. Una dependencia nueva se anota en `Cargo.toml` con su razón.

**Diferencial con Python (fase 0).** En una máquina con Python (la Mac):

```bash
python3 scripts/engine_golden.py            # regenera (sólo a propósito; revisar el diff)
python3 scripts/engine_golden.py --check    # ¿siguen siendo las respuestas de Python las que están en git?
cd engine && TZ=America/Hermosillo cargo test --offline -- --ignored
```

El script escribe `tests/fixtures/engine-golden/recipe.json` (en git, ~165 KB): JSON canónico, `recipe_ref`, resumen de `recipe`, familias de tarjeta, `discover` sobre 6 árboles y 7 tarjetas planeadas **y aplicadas** (estado final y copias verificadas). Es determinista en cualquier Mac: arma las tarjetas en `/tmp/nuun-engine-golden` y toma inode, ctime, dispositivo e ids del catálogo de contadores, con el mismo parche para el plan y el `apply` de Python. Las pruebas ignoradas fallan si el archivo falta o si algo difiere. Ejecutado por primera vez el 2026-11-11 en la Mac: las 5 pasan.

Una función nueva o portada agrega su caso aquí. Si cambia una respuesta de Python, `--check` lo detecta en CI antes de que Rust se quede atrás.

## Sombra y tiempos

El dorado prueba casos armados; la **sombra** prueba el catálogo real, sólo leyendo. Hay dos formas:

```bash
cd engine && cargo build --release --offline && cd ..
python3 scripts/engine_shadow.py replay              # las mismas lecturas a los dos motores, sin la app
python3 scripts/engine_shadow.py enable --engine engine/target/release/nuun-engine   # la app la usa al reabrir nüün
python3 scripts/engine_shadow.py report              # resumen de los últimos 7 días
python3 scripts/engine_shadow.py disable
```

- `replay` abre `python -S nu9ve.py media rpc-serve` y `nuun-engine rpc-serve` sobre el mismo catálogo y pregunta lo mismo por los dos tubos: cada lectura sin parámetros, más `job` para los últimos jobs y `library` por proyecto. Así los tiempos se comparan igual (de escribir la petición a leer la respuesta).
- `enable` escribe `<catálogo>/shadow.json`. El `rpc-serve` de Python que lanza la app responde como siempre y **después**, en un hilo, repite la lectura en Rust ([media/shadow.py](../media/shadow.py)). La sombra no cambia, retrasa ni hace fallar la respuesta; si la cola se llena, descarta. `NUUN_SHADOW_ENGINE=off` la apaga aunque exista el archivo. Las escrituras nunca pasan por la sombra.
- Los dos anotan en `<catálogo>/shadow/shadow-<fecha>.ndjson`: método, resultado (`same`, `diff` con las rutas JSON que difieren, `not_ported`, `same_error`, `error_mismatch`, `shadow_failed`) y los dos tiempos. Ese archivo es local y no guarda las respuestas completas.

**Medido el 2026-11-12, con el backend de macOS**, con `replay --repeat 5` sobre el catálogo real de la Mac (Apple Silicon, binario release, MEDIA502 y ESSD montados; mediana en ms, del pedido a la respuesta). **Todo lo portado responde igual que Python**, incluido el `snapshot` completo:

| Método | Python | Rust | |
| --- | --- | --- | --- |
| `snapshot` (vista principal) | 1645 | **601** | 2,7× |
| `source_cards` | 1170 | **343** | 3,4× |
| `suggest` (25 consultas) | 491 | **259** | 1,9× |
| `volumes` | 467 | **244** | 1,9× |
| `capture_overview` | 261 | 236 | |
| `closure_candidates` (6) | 817 | **292** | 2,8× |
| `missions_review` (6) | 539 | **373** | 1,4× |
| `storage_overview` | 638 | **395** | 1,6× |
| `storage_tree` (36, índice de 205 mil archivos) | 1,5 | 0,8 | |
| `closure_deliveries` (10) | 219 | 207 | |
| `integration_capabilities` | 545 | 533 | (es Clipper) |
| `projects` | 44,7 | **17,3** | 2,6× |
| `library` (10) | 40,1 | 36,4 | |
| `jobs`, `release_plans`, `job` | 0,4–8,5 | 0,6–9,7 | |
| Lecturas pequeñas (`recipe`, `closure_*`, `asset_revisions`…) | 0,03–0,32 | 0,01–0,63 | |
| Arranque por llamada (`rpc` de un solo uso) | ~80 | **~5** | 16× |

Cómo leerlo: lo que más tarda no es el motor sino lo que consulta afuera. `diskutil` tarda ~0,13 s por disco, y Clipper ~0,5 s. Rust gana porque hace esas consultas **al mismo tiempo**: el `snapshot` dura lo que tarda la más lenta, mientras que en Python dura la suma de todas. En lecturas pequeñas los dos responden en menos de 1 ms y no hay nada que ganar. El arranque importa en cada escritura, porque la app lanza un proceso `rpc` por mutación. La primera ejecución de un binario recién compilado tarda ~340 ms (macOS lo revisa una vez) y no cuenta.

Errores que la sombra encontró en Rust, ya corregidos: `projects` ordenaba los empates de `updated_at` por id, y Python conserva el orden de la biblioteca; `living_collections` faltaba en la lista de lecturas y Rust la rechazaba como escritura en vez de decir `not_ported`. [tests/test_engine_boundary.py](../tests/test_engine_boundary.py) mantiene esa lista igual a la de `media/cli.py`.

## Desde la app de Mac

Desde el 2026-11-12 la app usa Rust para lecturas y, dentro del sandbox, también para las escrituras que declara el motor.

- `PersistentReader` (`native/MediaMenu/Sources/MediaMenuCore/Protocol.swift`) arranca `nuun-engine rpc-serve` y le pregunta una vez `engine_info`. Cada lectura que está en `ported` va primero a Rust. Si Rust responde `not_ported`, `reader_unavailable` o `runtime_missing`, o el proceso se cae, la misma lectura va al `rpc-serve` de Python. Cualquier otro error es la respuesta del método.
- El motor se busca en `Contents/Resources/backend/nuun-engine` (lo pone `scripts/build_media_app.py`, firmado aparte porque la app no se firma con `--deep`) o, desde el código, en `engine/target/release/nuun-engine` junto a `nu9ve.py`.
- Ajustes → Diagnóstico → «Lecturas» elige `rust` (predeterminado si hay motor) o `python` (`UserDefaults` `readEngine`), y lista qué motor respondió cada método.
- En el sandbox, `BackendClient` consulta `engine_info.writes`. Para una escritura ya enviada a Rust, sólo cambia a Python ante `not_ported`; si el proceso se corta, el error queda a la vista y el diario permite reanudar un ingreso empezado. En el catálogo de uso diario, las escrituras siguen en Python.
- La sombra de Python sólo ve lo que Python responde. Para seguir comparando con la app en Rust, `scripts/engine_shadow.py replay` sigue sirviendo igual.

## Sandbox de macOS

El script monta imágenes ExFAT, HFS+ y FAT32, crea un catálogo aislado y carga contenido sintético. En ambos motores, una imagen cuenta como tarjeta o destino del sandbox cuando su archivo pertenece al entorno marcado con `.nuun-sandbox` y el catálogo efectivo está dentro de ese entorno. La app indica «Entorno de prueba» y apunta las integraciones a un `HOME` de prueba.

```sh
python3 scripts/sandbox.py status
(cd engine && cargo build --release --locked --offline)
python3 scripts/sandbox.py compare sony
python3 scripts/sandbox.py compare drone
python3 scripts/sandbox.py compare drone --no-cleanup
python3 scripts/sandbox.py compare drone --with-mission
python3 scripts/sandbox.py open --app /ruta/a/nuun-de-prueba.app
```

`compare` vacía las imágenes de destino y reinicia el catálogo del sandbox seleccionado. Para conservar otro entorno de prueba, crea uno aparte con `create --root /ruta/al/sandbox` y pasa el mismo `--root` a cada comando. También puedes usar `compare gopro`, `compare h6` y `compare canon`. Se comprueba el mismo documento de plan y su digest contra la misma tarjeta y el mismo catálogo antes de aplicar. Se comparan estado de `apply` y `resume`, rutas y SHA-256 de media, y el contenido de `units.ndjson` tras verificar sus referencias al job y a las unidades. El caso DJI con misión carga un registro sintético de Atlas. Estas imágenes comprueban el comportamiento del sistema de archivos y del motor. Las pruebas de extracción, lector físico y formato en cámara quedan en [release](../docs/release.md).

## Desde la ventana de Windows

`native/windows/Nuun` copia `engine\target\<debug|release>\nuun-engine.exe` a `engine\nuun-engine.exe` junto a la app, como nove con `engine\nove.exe`. Al abrir, la ventana elige el motor así:

1. Python, si `NUUN_PYTHON` y `NUUN_ENGINE` están definidas.
2. Si no, el motor de Rust (o el que diga `NUUN_ENGINE_BIN`), **método por método**: lo que Rust no tiene lo responde la maqueta (`MethodByMethodEngine`).
3. Si no, sólo la maqueta.

El catálogo es el de `--state-dir`, o `NU9VE_MEDIA_HOME`, o `%LOCALAPPDATA%/n-creator-utils/media` en Windows (en macOS, la misma carpeta que usa Python); el motor lo abre sólo para leer. Lo leído de verdad (destinos y tarjetas montadas) aparece aparte en Biblioteca → Discos, rotulado «sólo lectura»: esas tarjetas no entran al Ingreso simulado de la maqueta. Con `snapshot` portado, la ventana muestra los datos reales de este PC y deja de decir «Maqueta»; desde ese momento **planear y aplicar nunca caen a la simulación** (`MethodByMethodEngine.RealSnapshot`): un plan simulado sobre una tarjeta real parecería un respaldo que no ocurrió. `--maqueta` fuerza los datos ficticios para demos y para `qa-skins.ps1`. Ajustes → Motor dice qué respondió Rust. `Nuun.Core.Tests/RustEngineContractTests.cs` prueba el binario real; si no está compilado, la prueba sale como omitida y dice por qué.
