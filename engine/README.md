# Motor de nüün en Rust

Adoptado el 2026-11-09 ([análisis y ruta](../docs/platforms/rust-engine.md), [PLAN](../PLAN.md)). Habla el mismo protocolo que `python -S nu9ve.py media rpc-serve`: una línea JSON por petición `{id, method, params}` y una por respuesta, `{id, ok: true, data}` o `{id, ok: false, error: {code, message}}`. Python (`media/`) es el **oráculo** hasta que cada método llegue a paridad. Un método de lectura que todavía no está portado responde `not_ported`, y el cliente pregunta a otro motor.

| Crate | Qué tiene | Origen en Python |
| --- | --- | --- |
| `crates/nu9ve-json` | JSON canónico **byte a byte** igual al de Python (claves ordenadas, sin espacios, escapes de Python, `repr` de los decimales: `1e-07`, `1e+16`, `1.0`), digest SHA-256, error `{code, message}`, sobres de `rpc-serve` | `media/common.py` `canonical`, `digest`, `MediaError`; `media/cli.py` `serve_reads` |
| `crates/nu9ve-volumes` | Identidad de volumen en Windows (GUID, serial, discos físicos, bus, medio extraíble, índice de la raíz, espacio); letras montadas. Sólo lectura, sin administrador | curado `identity.rs`/`relink.rs` (Windows); `media/volumes.py` (Mac, sigue en Python) |
| `crates/nuun-recipe` | La receta como datos (`template.json`, transcripción de `TEMPLATE`), receta efectiva, familias de tarjeta por las dos primeras carpetas (contenido primero), resumen de `recipe` | `media/recipes.py`, `media/volumes.py` `card_hint` |
| `crates/nuun-media` | Catálogo SQLite: lectura sin crear nada; escritura sólo para `register_volume`, con el esquema de Python y el lock de escritor (`writer.lock`, el mismo `flock` que Python en Mac). Reglas de `slug` y rutas relativas, identidad, independencia física, destinos, `volumes`, `source_cards` y `card_hint` | `media/catalog.py`, `media/volumes.py`, `media/service.py` |
| `nuun-engine` | Binario `nuun-engine [--state-dir DIR] rpc-serve \| rpc \| --version` | `media/cli.py` |

Los `nu9ve-*` son candidatos a compartirse con nove, curado y kyu ([§6 del análisis](../docs/platforms/rust-engine.md#6-qué-compartir-en-rust-entre-las-apps)); los `nuun-*` son de nüün.

## Métodos

| Método | Estado | Prueba |
| --- | --- | --- |
| `recipe` | **Portado**: la receta guardada en el catálogo (o la plantilla) y los dispositivos confirmados, como Python | Unitarias; contrato desde C#; diferencial con Python **pendiente** del dorado |
| `volumes` | **Portado en Windows**: cada destino registrado con `online` (identidad comparada por `mode`, `volume_uuid`, `disk_uuid`, `root_inode`). En macOS, `not_ported` (sigue en Python) | Pruebas portadas de `test_media_engine`/`test_media_eject` con un backend de fixture; el volumen del sistema registrado como destino con el backend real; contrato desde C# |
| `source_cards` | **Portado en Windows**: unidades extraíbles y externas, nunca un destino ni un volumen en el mismo disco físico, ni imágenes. `hint` (familia, recomendación, uso, último proyecto) **sólo** con un destino en línea. Falta `hint.device` (EXIF, `photo_meta` sin portar) | Igual que `volumes`, más el diferencial de familias sobre árboles generados por Python (pendiente del dorado) |
| `engine_info` | Nuevo, sólo Rust: qué motor respondió y qué métodos tiene | Unitaria + contrato |
| Resto de `READ_METHODS` | `not_ported` | Unitaria |
| `register_volume` | **Portado en Windows**, sólo por `rpc` (un proceso por escritura, como la app de Mac con Python). Configuración, nunca media: describe el volumen (externo salvo `allow_local`), toma el lock, valida rol, nombre, prefijo y duplicados, y guarda el registro canónico con un UUID v4 | Pruebas portadas de los rechazos de Python; lock ocupado → `job_busy`; esquema y WAL del catálogo creado; contrato desde C#; registro desde la ventana por UI Automation |
| Otras mutaciones (`apply`, `plan`…) | `read_only` por `rpc-serve`; `not_ported` por `rpc` | Unitaria |

Siguiente, en orden: `snapshot` (jobs, proyectos, perfiles y defaults leídos del catálogo), búsqueda de biblioteca y, después, el planner con el digest del plan como oráculo. **Sin comprobar:** que Python abra un catálogo creado por Rust (mismo esquema, pero no se ha cruzado en la Mac).

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

El catálogo es el de `--state-dir`, o `NU9VE_MEDIA_HOME`, o `%LOCALAPPDATA%/n-creator-utils/media` en Windows (en macOS, la misma carpeta que usa Python); el motor lo abre sólo para leer. Lo leído de verdad (destinos y tarjetas montadas) aparece aparte en Biblioteca → Discos, rotulado «sólo lectura»: esas tarjetas no entran al Ingreso simulado de la maqueta. Ajustes → Motor dice qué respondió Rust. `Nuun.Core.Tests/RustEngineContractTests.cs` prueba el binario real; si no está compilado, la prueba sale como omitida y dice por qué.
