# Motor de nüün en Rust

Adoptado el 2026-11-09 ([análisis y ruta](../docs/platforms/rust-engine.md), [PLAN](../PLAN.md)). Habla el mismo protocolo que `python -S nu9ve.py media rpc-serve`: una línea JSON por petición `{id, method, params}` y una por respuesta, `{id, ok: true, data}` o `{id, ok: false, error: {code, message}}`. Python (`media/`) es el **oráculo** hasta que cada método llegue a paridad. Un método de lectura que todavía no está portado responde `not_ported`, y el cliente pregunta a otro motor.

| Crate | Qué tiene | Origen en Python |
| --- | --- | --- |
| `crates/nu9ve-json` | JSON canónico **byte a byte** igual al de Python (claves ordenadas, sin espacios, escapes de Python, `repr` de los decimales: `1e-07`, `1e+16`, `1.0`), digest SHA-256, error `{code, message}`, sobres de `rpc-serve` | `media/common.py` `canonical`, `digest`, `MediaError`; `media/cli.py` `serve_reads` |
| `crates/nu9ve-volumes` | Identidad de volumen en Windows (GUID, serial, discos físicos, bus, medio extraíble, índice de la raíz, espacio); letras montadas, sin administrador. Módulo `fs` para quien mueve media: sync completo (`F_FULLFSYNC`), sync de carpeta, renombrado que nunca reemplaza, apertura sin seguir enlaces, lectura sin caché en Mac, rutas largas `\\?\` | curado `identity.rs`/`relink.rs` (Windows); `media/volumes.py` (Mac, sigue en Python) |
| `crates/nu9ve-time` | Tiempo civil con la semántica de `datetime` de Python: `fromisoformat`, `isoformat`, `strftime` (C locale), `fromtimestamp` con redondeo a par y `astimezone()` con el algoritmo de CPython (`local_to_seconds`: pliegues y huecos de horario de verano). En Mac la hora local sale de `localtime_r`, la misma de Python, y respeta `TZ`; en Windows, de la zona del sistema (`chrono`) | `datetime`, `media/evidence.py` |
| `crates/nuun-recipe` | La receta como datos (`template.json`, transcripción de `TEMPLATE`), receta efectiva, familias de tarjeta por las dos primeras carpetas (contenido primero), resumen de `recipe` | `media/recipes.py`, `media/volumes.py` `card_hint` |
| `crates/nuun-media` | Catálogo SQLite: lectura sin crear nada; escritura sólo para `register_volume`, con el esquema de Python y el lock de escritor (`writer.lock`, el mismo `flock` que Python en Mac). Reglas de `slug` y rutas relativas, identidad, independencia física, destinos, `volumes`, `source_cards` y `card_hint`; escaneo de tarjetas, EXIF/XMP de fotos y dispositivo por receta; misiones (índice y match), nombres de destino (`layout`), lecturas seguras bajo una raíz (`tree`) y el planner de tarjetas | `media/catalog.py`, `media/volumes.py`, `media/service.py`, `media/planner.py`, `media/photo_meta.py`, `media/recipes.py`, `media/missions.py`, `media/layout.py`, `media/common.py` `Tree` |
| `nuun-engine` | Binario `nuun-engine [--state-dir DIR] rpc-serve \| rpc \| --version` | `media/cli.py` |

Los `nu9ve-*` son candidatos a compartirse con nove, curado y kyu ([§6 del análisis](../docs/platforms/rust-engine.md#6-qué-compartir-en-rust-entre-las-apps)); los `nuun-*` son de nüün.

## Métodos

| Método | Estado | Prueba |
| --- | --- | --- |
| `recipe` | **Portado**: la receta guardada en el catálogo (o la plantilla) y los dispositivos confirmados, como Python | Unitarias; contrato desde C#; diferencial con Python **pendiente** del dorado |
| `volumes` | **Portado en Windows**: cada destino registrado con `online` (identidad comparada por `mode`, `volume_uuid`, `disk_uuid`, `root_inode`). En macOS, `not_ported` (sigue en Python) | Pruebas portadas de `test_media_engine`/`test_media_eject` con un backend de fixture; el volumen del sistema registrado como destino con el backend real; contrato desde C# |
| `source_cards` | **Portado en Windows**: unidades extraíbles y externas, nunca un destino ni un volumen en el mismo disco físico, ni imágenes. `hint` (familia, recomendación, uso, último proyecto) **sólo** con un destino en línea. `hint.device` del EXIF de la primera foto bajo DCIM (`card_device`, a lo más 64 entradas), que cambia la recomendación de un dispositivo conocido | Igual que `volumes`, más el diferencial de familias sobre árboles generados por Python (pendiente del dorado) |
| `snapshot` | **Portado**: `volumes` (con `online` y `free`), `library`, `jobs` (con la etapa Sony), `profiles`, `projects`, `release_plans`, `integration_settings`, `defaults`, `project_closures` y `source_cards`. Lo que falta se nombra en `errors`, como Python nombra una pieza que falla: `capabilities` (lanza Clipper y Stalker), `sony_removal_recorded` de un job Sony fallido al leer la tarjeta, y en macOS el estado en línea y las tarjetas | Unitarias con un catálogo de prueba (jobs, biblioteca, proyectos sin miniaturas, cierres); contrato desde C#. Diferencial con Python pendiente |
| `discover` | **Portado para tarjetas**, sólo por `rpc` (como Python). Recorrido propio sin seguir enlaces ni puntos de reanálisis (el de Python usa `O_NOFOLLOW`/`O_DIRECTORY` y no corre en Windows), archivos ignorados, residuos de Finder, validación de bundles Stalker, evidencia de cámara (incluido el XML de Sony), agrupado en unidades. Lee EXIF/XMP de JPG y DNG (`photo_meta`, sin dependencias) y da `device` (registro confirmado, regla de la receta, evidencia de vuelo o «por confirmar») y `created_at` (hora de cámara). Con `archive`, `mission` contra el registro de misiones del archivo (mismo nombre, hora de cámara y posición; sin archivo en línea, nada, como Python); donde no hay backend de volúmenes (macOS hoy) lo dice en `not_ported`. Igual que Python tras corregir allí un error (ver abajo). Una fuente con sesiones Stalker responde `not_ported` (sus metadatos siguen en `captures.py`). Colisiones de nombre sin NFC (sin crate de normalización); un caso no detectado igual falla seguro al renombrar | Unitarias de escaneo, EXIF (JPEG y TIFF sintéticos, archivos dañados, redondeo de Python) y dispositivo; contrato desde C# con un JPEG con EXIF; diferencial sobre 6 árboles de tarjeta generados por Python, pendiente del dorado (sin EXIF) |
| `jobs`, `job`, `library`, `projects`, `profiles`, `devices`, `release_plans`, `defaults`, `integration_settings` | **Portados**: las mismas lecturas que `snapshot` junta, como métodos propios. `library` filtra por proyecto (`casefold` para títulos: minúsculas, `ß`→`ss`) y estado de trabajo; `job` da la fila completa, el plan y el progreso. `jobs` responde `not_ported` entero si un job Sony fallido necesita `sony_removal_recorded`, antes que una respuesta incompleta; `defaults` e `integration_settings` sólo sin parámetros (con parámetros son escrituras) | Unitarias con el esquema completo de Python |
| `engine_info` | Nuevo, sólo Rust: qué motor respondió y qué métodos tiene | Unitaria + contrato |
| Resto de `READ_METHODS` | `not_ported` (`capture_*`, `closure_*`, `storage_*`, `suggest`, `asset_revisions`, `project_preview`, `missions_review`, `missions_status`, `integration_capabilities`, `action_progress`) | Unitaria |
| `register_volume` | **Portado en Windows**, sólo por `rpc` (un proceso por escritura, como la app de Mac con Python). Configuración, nunca media: describe el volumen (externo salvo `allow_local`), toma el lock, valida rol, nombre, prefijo y duplicados, y guarda el registro canónico con un UUID v4 | Pruebas portadas de los rechazos de Python; lock ocupado → `job_busy`; esquema y WAL del catálogo creado; contrato desde C#; registro desde la ventana por UI Automation |
| `plan`, `plan_document` | **Portados para tarjetas en Windows**, sólo por `rpc`. `_build_plan` paso a paso y en el orden de Python (el primer chequeo que falla decide el código): política de sólo-archivo, identidad del origen, discos físicos distintos, solapes, escaneo, asignaciones (proyecto, perfil, país, cámara, trabajo, descarte), dispositivo por EXIF, misión contra el archivo, copias ya presentes en la raíz del consumidor (`WorkPresence`), fotos ya respaldadas, `layout` (nombres de Python: hora de cámara, XML de Sony, WAV, ffprobe si está, mtime en la zona del sistema), ids `uuid5` y `digest`, vista previa, conflictos de destino, `validate_plan` y `preflight` con espacio. Guarda `reviewed-plan:<id>` bajo el lock. `stalker` y `delivery` (y la ruta de finales) responden `not_ported` | Pruebas portadas de `test_devices` y `test_mission_match` (nombres de dron, misión emparejada, DJI que pregunta, par de dos dispositivos, códigos de política, descartes, solape, mismo disco, lock ocupado, plan cambiado); en el PC, `plan` real por el binario con una carpeta local; diferencial de 7 tarjetas contra Python **pendiente** del dorado |
| `apply`, `resume` | **Portados para tarjetas en Windows**, sólo por `rpc`, bajo el lock de escritor. `Transfer.apply` de Python paso a paso: cada copia va a `.<nombre>.ncpartial` (una lectura del origen escrita en paralelo a cada destino), se sincroniza con su carpeta (`F_FULLFSYNC` en Mac), se relee y se compara por SHA-256 antes de renombrarla **sin reemplazar nunca** (`MoveFileExW` sin reemplazo; `renamex_np` exclusivo en Mac). Un destino con los mismos bytes se adopta; uno distinto es `destination_conflict`. Diario en `members`/`copies` por archivo: un job interrumpido se reanuda y toma una réplica verificada en vez de volver a leer la tarjeta. Recibos en `.ncreator/receipts/`, índice (`assets`, `work`, proyectos), limpieza sólo tras verificar todo, reescanear y **releer cada original** con el mismo hash (los índices Sony nunca se borran), descartes al final, `source_has_new_material` si apareció algo. Después, `units.ndjson` de misiones y respaldo del catálogo en el archivo (`VACUUM INTO`, verificado). `stalker`, `delivery` y planes de esquema 1 responden `not_ported` sin tocar nada. Rutas de más de 260 caracteres en Windows (forma `\\?\`) | Pruebas portadas de `test_media_engine` (dos destinos y limpieza, reintento idempotente, corrupción del mismo tamaño, copia interrumpida, caída antes de renombrar, limpieza parcial reanudada, archivo nuevo antes y durante la limpieza, destino ajeno, destino idéntico adoptado, origen cambiado, desconexión antes y durante la limpieza, corrupción durante la limpieza, Sony conserva índices, descartes, plan cambiado o cancelado, lock ocupado, `resume` y respaldo del catálogo) y de `test_mission_match` (misión → carpetas y `units.ndjson`); en el PC, `plan` → `apply` → `resume` por el binario; el dorado aplica cada plan y compara estado y copias verificadas con Python |
| Otras mutaciones | `read_only` por `rpc-serve`; `not_ported` por `rpc` (`confirm_device`, `confirm_camera_cleanup`, `reconcile_sony_cleanup`, `eject_card`, `recover_catalog`, releases, finales…) | Unitaria |

**Error de Python encontrado al portar y corregido** (`media/service.py`, `discover`, 2026-11-09): `metadata_device` sólo se asignaba si había dispositivo y se borraba si no había hora de cámara. Una foto con dispositivo y sin `DateTimeOriginal` perdía su `device`; una foto fechada sin `Make` heredaba el `device` de la unidad anterior, o lanzaba `UnboundLocalError` si era la primera. Ahora cada unidad empieza vacía, en Python y en Rust. Prueba nueva en `tests/test_devices.py` (`test_discover_keeps_each_units_device_and_camera_time_apart`), **sin ejecutar**: no hay Python en este PC.

**Dorado en la Mac** (lo que valida todo lo de arriba contra Python): `python3 scripts/engine_golden.py`, luego `cd engine && TZ=America/Hermosillo TMPDIR="$(cd "$TMPDIR" && pwd -P)" cargo test --offline -- --ignored`. `TZ` porque los nombres sin hora de cámara usan la zona del sistema; `TMPDIR` resuelto porque las carpetas temporales de macOS cuelgan de `/var`, un enlace que el escaneo rechaza. En la Mac, `cargo` necesita descargar una vez `libc` si no está en su caché.

**Cambio de identidad en Windows (2026-11-10):** `root_inode` es ahora el índice de la **carpeta registrada**, como Python usa el `stat` de la raíz que recibe (antes era el de la raíz de la unidad). Un destino registrado antes en Windows aparece fuera de línea (`volume_replaced`) y se registra de nuevo; sólo había registros de QA.

Siguiente, en orden: una **corrida física explícita** de `apply` con una SD y un disco USB reales ([release gates](../docs/release.md)), llevar `plan`/`apply` a la ventana de Windows, la confirmación de limpieza de cámara (`confirm_camera_cleanup`, `reconcile_sony_cleanup`) y `eject_card`, `recover_catalog`, `stalker`/`delivery`, `capabilities`. Sin cancelación desde el cliente todavía: matar el proceso deja el job reanudable (Python lo marcaría `paused`). **Sin comprobar:** que Python abra un catálogo creado por Rust (mismo esquema, pero no se ha cruzado en la Mac).

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
```

Las dependencias son las mismas versiones que nove y curado y ya estaban en la caché local: `serde`, `serde_json`, `sha2`, `regex` (las condiciones `dcim_regex` son datos editables), `windows-sys` y `tempfile` para las pruebas. Ninguna se descargó. Una dependencia nueva se anota en `Cargo.toml` con su razón.

**Diferencial con Python (fase 0).** En una máquina con Python (la Mac):

```bash
python3 scripts/engine_golden.py
cd engine && cargo test --offline -- --ignored
```

El script escribe `tests/fixtures/engine-golden/recipe.json` con casos de JSON canónico, el `recipe_ref` de la plantilla y el resumen de `recipe`. Las pruebas ignoradas fallan si el archivo falta o si un solo byte difiere. **El script no se ha ejecutado todavía**: esta máquina Windows no tiene Python.

## Desde la ventana de Windows

`native/windows/Nuun` copia `engine\target\<debug|release>\nuun-engine.exe` a `engine\nuun-engine.exe` junto a la app, como nove con `engine\nove.exe`. Al abrir, la ventana elige el motor así:

1. Python, si `NUUN_PYTHON` y `NUUN_ENGINE` están definidas.
2. Si no, el motor de Rust (o el que diga `NUUN_ENGINE_BIN`), **método por método**: lo que Rust no tiene lo responde la maqueta (`MethodByMethodEngine`).
3. Si no, sólo la maqueta.

El catálogo es el de `--state-dir`, o `NU9VE_MEDIA_HOME`, o `%LOCALAPPDATA%/n-creator-utils/media` en Windows (en macOS, la misma carpeta que usa Python); el motor lo abre sólo para leer. Lo leído de verdad (destinos y tarjetas montadas) aparece aparte en Biblioteca → Discos, rotulado «sólo lectura»: esas tarjetas no entran al Ingreso simulado de la maqueta. Con `snapshot` portado, la ventana muestra los datos reales de este PC y deja de decir «Maqueta»; desde ese momento **planear y aplicar nunca caen a la simulación** (`MethodByMethodEngine.RealSnapshot`): un plan simulado sobre una tarjeta real parecería un respaldo que no ocurrió. `--maqueta` fuerza los datos ficticios para demos y para `qa-skins.ps1`. Ajustes → Motor dice qué respondió Rust. `Nuun.Core.Tests/RustEngineContractTests.cs` prueba el binario real; si no está compilado, la prueba sale como omitida y dice por qué.
