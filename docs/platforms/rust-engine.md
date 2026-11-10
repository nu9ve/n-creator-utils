# ¿Pasar el motor de nüün a Rust?

Análisis del 2026-11-09, pedido por el usuario. **Adoptado por el usuario el 2026-11-09** (ver [PLAN](../../PLAN.md)). La implementación empieza en [`engine/`](../../engine/README.md). Mediciones hechas en esta máquina (Windows 11, AMD Ryzen 5 3550H). Lo que no se midió se dice.

## 1. Respuesta corta

- **Sí conviene, pero no por velocidad de verificación.** Copiar y verificar están limitados por los discos, no por el lenguaje. Medido: SHA-256 en un hilo va a **1,67 GB/s tanto en Rust (`sha2`) como en .NET (CNG)**, y `hashlib` de Python usa OpenSSL con las mismas instrucciones del procesador. Una SD lee entre 90 y 250 MB/s y un disco USB entre 150 y 1000 MB/s, así que el hash es entre 2 y 18 veces más rápido que el disco que lo alimenta. El motor de Python además ya hace lo caro bien: lee la tarjeta una sola vez, reparte cada bloque en paralelo a todos los destinos, solapa la lectura con la escritura y verifica sin caché (`transfer.py` `copy_many`).
- **Lo que sí mejora con Rust:**
  1. Un binario firmado sin Python externo, que es un gate de [release](../release.md) en Mac y el bloqueo principal en Windows.
  2. El mismo motor en Windows y en Mac ([windows-app-v1 §0](../contracts/windows-app-v1.md#0-principio-dos-apps-nativas-un-producto)).
  3. Arranque inmediato de los procesos por mutación.
  4. Menos sobrecarga por archivo en miles de archivos chicos (miniaturas, DNG, sidecars).
  5. Contratos con tipos que generan sus esquemas.
  6. Compartir piezas con nove, curado y kyu, que ya son Rust.
- **Windows primero en Rust y luego Mac sí es viable**, con una condición: el motor de Rust es **el** motor de nüün, no «el de Windows». Mientras no tenga paridad, Python es la referencia (el oráculo) y la app de Mac cambia de motor **método por método**, cuando las pruebas diferenciales y una corrida física lo respaldan. Dos motores con lógica distinta por plataforma es justo lo que el contrato prohíbe.

## 2. Qué hay hoy

| | Medida |
| --- | --- |
| Motor de media (`media/`) | 9 139 líneas de Python en 32 módulos; los más grandes: `closure` 991, `storage_index` 732, `transfer` 722, `missions` 604, `planner` 591, `service` 582 |
| Pruebas | 362 en Python (8 nov), con volúmenes de fixture y puntos de falla (`checkpoint('copy_block')`, `before_verify`) |
| Dependencias de Python | Ninguna (`dependencies = []`); SQLite del sistema, `hashlib`, `fcntl` |
| Cosas de Mac | `volumes.py`: `diskutil info/list/eject`, `F_NOCACHE`, `F_FULLFSYNC` |
| Procesos desde la app | Un lector persistente `rpc-serve` para lecturas; **un proceso por mutación** (`rpc`) |
| Descargas y stems (`manager/`) | Llaman a `yt-dlp` y `demucs` como programas externos; no dependen de librerías de Python |
| Rust en la suite | nove `core` ~29 000 líneas (pipe/socket, MCP, SQLite, jobs); curado ~20 800 (audio, biblioteca, identidad de volumen en Windows); kyu `kyu-core` por FFI |

## 3. Rendimiento: dónde se va el tiempo

Una copia verificada de N bytes a D destinos cuesta, en el mejor caso, leer N de la tarjeta, escribir N en cada destino **en paralelo** y volver a leer N de cada destino para verificar. Esa segunda lectura es la garantía: no se quita sin debilitar «verificado».

| Paso | Lo limita | ¿Rust lo acelera? |
| --- | --- | --- |
| Leer la tarjeta y calcular el hash | Velocidad de la SD (90–250 MB/s) | No: el hash va a 1,67 GB/s en ambos (medido) |
| Escribir en MEDIA y ESSD | El destino más lento | No; ya va en paralelo por destino |
| Verificar (releer cada destino) | Lectura de cada destino; ya en paralelo | No en bytes. Sí, un poco, si se verifica un archivo mientras se copia el siguiente en **otro** disco (también posible en Python) |
| `fsync` del archivo y de su carpeta por archivo | La latencia del disco | No por el lenguaje. Sí agrupando el `fsync` de la carpeta en lotes, otra vez en cualquier lenguaje |
| Miles de archivos chicos | Sobrecarga por archivo: `open`/`stat`/`rename` y el intérprete | **Sí, algo:** el intérprete pesa cuando el archivo es chico |
| Arranque de cada mutación | Importar Python y abrir el catálogo, cada vez | **Sí:** un binario arranca en milisegundos. No medido aquí (no hay Python en esta máquina) |
| Escaneo y discovery de tarjetas grandes | `scandir`, EXIF, lectura de manifiestos | Sí en CPU, si el escaneo resulta ser CPU y no disco. Medir primero |

**Conclusión:** para ingresar una SD de 64 GB la diferencia entre lenguajes es marginal: manda el disco. Para mantener la app ágil (arranque, cientos de miniaturas, catálogos grandes) y, sobre todo, para distribuirla, Rust sí ayuda. Antes de prometer números: medir cada fase con la medida que ya existe (`ActionMeter`, con bytes por segundo de lectura, escritura y verificación) en una corrida física pedida por el usuario.

**No cambiar SHA-256 por BLAKE3** para ganar velocidad. El hash está en recibos, contratos y catálogo; no es el cuello de botella, y cambiarlo rompe la compatibilidad con todo lo ya verificado.

## 4. Qué se gana además del rendimiento

| Ganancia | Por qué importa en nüün |
| --- | --- |
| **Un binario, sin Python externo** | Hoy «Python externo» es un límite en [STATUS](../STATUS.md#build-y-pruebas); notarizar un runtime de Python es trabajo propio. En Windows, empaquetar Python es el bloqueo principal de la maqueta |
| **Un motor para dos apps** | Windows y Mac piden lo mismo por el mismo protocolo |
| **Contratos con tipos** | `serde` + `schemars` generan JSON Schema de cada respuesta, y de ahí se prueban o generan los DTOs de Swift y C# |
| **Piezas compartidas** | El volumen por serial de curado, el MCP y el pipe de nove, el lock de escritor único (§6) |
| **Seguridad de tipos en lo irreversible** | Limpieza, liberación y cierre son máquinas de estados; un `enum` exhaustivo atrapa en compilación un estado olvidado |

## 5. Riesgos y costos

- **Reescribir lógica de seguridad de datos:** reglas Sony, bundles Stalker, cierre editorial, A11–A16 de dron y diario de limpieza. Un error ahí borra media. Por eso el orden es lectura → plan → copia → limpieza, y cada paso pasa las pruebas diferenciales (§7) antes del siguiente.
- **Tiempo:** 9 mil líneas de Python suelen dar entre 15 y 20 mil de Rust con sus pruebas; es del orden de la mitad del motor de nove. Durante la migración conviven dos motores.
- **Integraciones:** Stalker (manifiestos) y Clipper (CLI de relink/ACK) no cambian: son archivos y procesos, no Python.
- **Lo que no se migra:** descargas y stems pueden quedarse en Python o invocarse desde Rust igual que hoy (`yt-dlp`, `demucs` son programas). curado ya hace esto con Demucs.

## 6. Qué compartir en Rust entre las apps

Primero se copia con comentario de origen y se extrae a crate común al llegar a tres copias ([tema §10](../contracts/nu9ve-theme-v1.md#10-implementación-compartida)).

| Crate candidato | Origen hoy | Lo usaría |
| --- | --- | --- |
| `nu9ve-ipc`: JSON enmarcado por named pipe (ACL del SID, `FIRST_PIPE_INSTANCE`, PID del servidor) y socket Unix `0700` | nove `core` | nüün cuando su motor sea de larga vida (bandeja, segundo plano) |
| `nu9ve-mcp`: servidor stdio con niveles, anotaciones, `_untrusted`, salida acotada, diálogo nativo, permiso por sesión ([nu9ve-mcp-v1](../contracts/nu9ve-mcp-v1.md)) | nove `core/src/mcp.rs`, `agent.rs` | nüün, curado, kyu |
| `nu9ve-volumes`: identidad de volumen por serial (`GetVolumeInformationW`), letras, espacio libre (`GetDiskFreeSpaceExW`), equivalentes en Mac (`statfs`, DiskArbitration) | curado `identity.rs`/`relink.rs` (Windows); nüün `volumes.py` (Mac) | nüün (tarjetas y destinos), curado (USB de rekordbox), kyu |
| `nu9ve-journal`: escritura `.partial` → verificar → renombrar sin reemplazar, `fsync` completo, lock de escritor único, diario de operaciones | nüün `transfer.py`/`catalog.py`; nove (lock de raíz, `fs2`) | nüün, kyu (entrega de fotos), Clipper si migra |
| `nu9ve-json`: JSON canónico y digest, sobre `{code, message}` de errores | nüün `common.py` (`canonical`, `digest`); nove | Todas |

Prácticas que nove y curado ya aplican y conviene fijar para la suite:

- Workspace con `edition = "2024"` y `rust-version` declarado.
- `cargo fmt --check` y `clippy --all-targets -D warnings` en verde antes de cerrar.
- Pruebas con raíces temporales y fixtures; nunca discos personales.
- `rusqlite` con `bundled`.
- Dependencias fijadas y anotadas con su razón (curado: «no actualizar por rutina»; nove: una entrega por dependencia).
- `windows-sys` para las APIs de Windows en vez de crates grandes.
- `[profile.release]` con `lto = "thin"` donde importe.
- Elegir **una** librería de tiempo para la suite: nove usa hoy `jiff` y `chrono` juntas.

## 7. Ruta recomendada

| Fase | Trabajo | Criterio para pasar |
| --- | --- | --- |
| 0 · Medir y fijar | Tiempos por fase en una corrida física pedida; «respuestas doradas» de cada método RPC sobre los fixtures (`snapshot`, `source_cards`, `recipe`, `plan_document`) | Números por fase registrados en STATUS; JSON dorado en `tests/fixtures/contracts` |
| 1 · Lectura en Rust | `nuun-engine` con el mismo `rpc-serve` (una línea JSON, sobre `{ok, data}`): `volumes`, `source_cards`, `recipe`, `snapshot`, búsqueda de biblioteca. Adaptador de volúmenes de **Windows** y de Mac. MCP desde `nu9ve-mcp` | **Prueba diferencial:** los dos motores, mismos fixtures, mismo JSON. La maqueta de Windows deja de ser maqueta en sólo lectura |
| 2 · Planner | `plan` y `plan_document`, con la receta como datos | El **digest del plan** es idéntico en Python y en Rust para todos los fixtures (el oráculo más fuerte: JSON canónico) |
| 3 · Copia verificada | `copy_many`, `.ncpartial`, verificación, renombrar, diario, progreso `ActionMeter` | Mismas pruebas de falla que Python (cortar en `copy_block` y en `before_verify`); corrida física en Windows con plan revisado |
| 4 · Limpieza y liberación | Sony, Stalker con relink/ACK de Clipper, cierre, dron | Pruebas de cada regla portadas; corridas físicas por dispositivo |
| 5 · Mac cambia de motor | `BackendConfiguration` elige `rust` o `python` por método; Python queda un ciclo como respaldo y luego se retira de `media/` | Una semana de uso real en Mac sin diferencias; Python sólo para `legacy-save` y descargas, o también portado |

Reglas durante la migración:

- Cada método tiene **un** dueño a la vez. Mientras Python lo sea, las funciones nuevas entran en Python y la prueba diferencial obliga a portarlas. Cuando Rust lo sea, entran en Rust.
- La app nativa nunca sabe qué lenguaje respondió: mismo protocolo y mismos códigos de error.
- No se declara una fase lista por compilar: la evidencia es la diferencial y, desde la fase 3, la corrida física.

## 7b. Estado de la migración

Detalle por método en [`engine/README.md`](../../engine/README.md).

| Fecha | Hecho | Evidencia | Falta |
| --- | --- | --- | --- |
| 2026-11-09 | Workspace `engine/` con las convenciones de §6: `nu9ve-json` (canónico de Python, digest, errores), `nu9ve-volumes` (identidad por serial en Windows), `nuun-recipe` (receta y familias de tarjeta), `nuun-engine` (`rpc-serve`: `recipe` portado, el resto `not_ported`, mutaciones `read_only`) | Windows 11: 18 pruebas de Rust (`fmt` y `clippy -D warnings` limpios), incluidas las de `test_recipe_cards.py` portadas caso por caso y 18 decimales contra el `repr` de Python. 3 pruebas de contrato desde C# con el binario real (58 en total). La ventana de Windows lanza el motor, toma `recipe` de Rust y el resto de la maqueta; al cerrarla, el motor termina | Correr `scripts/engine_golden.py` en la Mac y las 2 pruebas diferenciales ignoradas. Medir cada fase en una corrida física. Leer el catálogo SQLite para `volumes`, `source_cards` y `snapshot` |

| 2026-11-09 | `nuun-media`: catálogo SQLite de sólo lectura (`rusqlite` embebido, como nove) y reglas de `volumes.py`. `volumes` y `source_cards` portados en Windows con la identidad `mode: "windows"` (diseño nuevo: Python no tenía backend de Windows). `recipe` lee la receta guardada y el registro de dispositivos | 31 pruebas de Rust; las de `test_media_eject.py`/`test_media_snapshot.py` sobre un backend de fixture; el backend real sobre el volumen del sistema (identidad completa, en línea como destino, `volume_replaced` si cambia); 4 de contrato desde C#. El script dorado ahora también registra familias sobre árboles de carpetas | Dorado en la Mac; una SD y un disco USB reales en Windows; `hint.device` (EXIF); `register_volume` |

| 2026-11-09 | Primera escritura: `register_volume` por `rpc` (un proceso por escritura). Crea el catálogo con el esquema de Python, toma `writer.lock` (`File::try_lock`, el mismo `flock` que Python en Mac) y guarda el registro canónico con UUID v4 del generador del sistema. Diálogo «Registrar destino…» en la ventana de Windows | 39 pruebas de Rust (rechazos de Python, `job_busy`, esquema y WAL, carpeta interna sólo con consentimiento); 60 de C#; recorrido por UI Automation en raíz de QA | Que Python abra el catálogo creado por Rust; diálogos con el skin de época; `snapshot` |

| 2026-11-09 | `snapshot` portado (todas las piezas de lectura del catálogo; `capabilities` y un caso Sony nombrados en `errors`). La ventana de Windows usa el snapshot real: sin «Maqueta», estados vacíos útiles, aviso de que respaldar aún no está en Windows, y planear o aplicar ya no caen a la simulación. `--maqueta` para demos y QA | 43 pruebas de Rust; 61 de C#; recorrido en raíz de QA: registrar destino y verlo en Inicio | `capabilities`; diferencial de `snapshot` contra Python; planner |

| 2026-11-09 | Planner, primer paso: `discover` de tarjetas (`scan`, evidencia de cámara con XML de Sony vía `quick-xml`, agrupado en unidades). Hallazgo: el escaneo de Python usa descriptores de carpeta que Windows no tiene, así que **el planner de Python sólo corre en macOS**; Rust trae su propio recorrido sin seguir enlaces ni puntos de reanálisis. En Ingreso, «Leer contenido» muestra lo que el motor leyó de una tarjeta real | 51 pruebas de Rust; 62 de C#; el dorado ahora incluye `discover` sobre 6 árboles de tarjeta | EXIF (`photo_meta`), `layout.py`, resto de `_build_plan`; probar «Leer contenido» con una tarjeta real |

| 2026-11-09 | EXIF/XMP de fotos (`photo_meta`: Make, Model, serial, hora y desfase, GPS, vuelo DJI; acotado, sin dependencias) y dispositivo por receta (`resolve`, `unit_device`, `card_device`). `discover` da `device` y `created_at`; `source_cards` da `hint.device` y cambia la recomendación. Ingreso muestra el dispositivo y cuántas unidades piden confirmarlo. Hallazgo: error en `discover` de Python con fotos sin hora o sin marca, corregido allí con una prueba nueva (ver [engine/README](../../engine/README.md#métodos)) | 62 pruebas de Rust (JPEG/TIFF sintéticos, dañados, redondeo de Python); 62 de C#, la de contrato con un JPEG con EXIF | `mission`; correr la prueba de Python nueva en la Mac; un dorado con fotos con EXIF; una tarjeta real |

## 8. Qué se puede mejorar ya, sin cambiar de lenguaje

1. Medir cada fase (bytes/s de lectura, escritura y verificación por dispositivo) en `ActionMeter` y en el recibo.
2. Un proceso persistente también para mutaciones, o al menos medir cuánto cuesta el arranque por petición.
3. Agrupar el `fsync` de carpeta en lotes de archivos chicos, con una prueba de corte que lo justifique.
4. Verificar un archivo en un destino mientras se copia el siguiente, sólo si son discos físicos distintos y midiendo antes (en un HDD las dos operaciones compiten por el cabezal).

Estas mejoras valen igual en Rust y conviene probarlas primero en Python, que es el oráculo.
