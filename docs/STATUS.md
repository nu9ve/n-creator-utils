# Estado vigente

Actualizado: 2026-11-26. **No es una release de producción.** Esta página dice qué funciona hoy, con qué evidencia y qué falta; la bitácora por entrega hasta el 8 nov está en [releases/status-log-2026-10-25_2026-11-08](releases/status-log-2026-10-25_2026-11-08.md). Prioridades en [PLAN](../PLAN.md), gates de distribución en [release](release.md).

Regla para mantenerla: al cerrar una entrega se **actualiza la fila de su área** (estado, evidencia, pendiente) y se agrega, como mucho, una entrada corta en «Entregas recientes». Cuando esa sección pase de ~5 entradas, las más viejas se mueven a la bitácora de `releases/`.

## Build y pruebas

| Qué | Valor |
| --- | --- |
| App instalada en la M1 | `~/Applications/nuun.app`, **0.2.0 / `c4350a4e0416`** (27 nov, desde `cdcfc3d`: core `b4dee11`; F4 a F7b, U1.3, U3, vectores de nombres y F5 (c)). Firmada con Apple Development y `codesign --verify --deep --strict` en verde; su `nuun-engine` responde `media` sobre un catálogo de prueba, así que Clipper, por el checkout, ya llega a Rust. El usuario la revisa en ventana y avisa si algo falla. Build anterior en `app-backups/67350776.app`. Antes, `f65bb6e27f56` (26 nov, desde `e6a5275`: core `6718418`, barra única del core y «Con tu color» por omisión; ejecutable `Nuun`). Antes, `21a4df30f435` (desde `89606dc`: F2 nombre `Nuun`, F3 `NuMacShell` y la barra única propia), firmada con Apple Development y `codesign --verify --deep --strict` en verde; sin aceptar en ventana. Antes, `59fd8fe38f08` (desde `2263b26`: progreso del respaldo de conversaciones, sugerencias sin ida y vuelta y Terminal centrado; [entrega](#acciones-y-sugerencias--2026-11-26)), firmada con Apple Development y `codesign --verify --deep --strict` en verde; pendiente aceptarla en ventana. Build anterior en `app-backups/9395a8b9.app`. Antes, `c0de130ed8e9` (desde `eac3ce7`: archivo de cancelación del motor y `_` → `-` en nombres nuevos) y `c2cc37cd3ec3`: popup a 127 ms y tarjeta completa a 1,97 s del montaje, fuentes en una fila con indicadores por tema y lector del motor en una pasada (`snapshot` al arrancar 1,2 s, antes 6–7 s; [PLAN](../PLAN.md#p0--uso-diario-sin-sorpresas)). Antes, `426a2eabd20b`: registro de misiones con aterrizajes y referencias, aviso de export en Ingreso y tiempos de llegada de tarjeta en el registro del sistema (`subsystem app.nu9ve.media`). **Firmada con Apple Development** desde el llavero (`install_media_app.py`), ya no ad-hoc; `codesign --verify --deep --strict` pasó. La primera apertura con esta firma puede pedir los permisos una vez. Build anterior en `app-backups/`. |
| App instalada en la M4 | `~/Applications/nuun.app`, **0.2.0 / `ccb5ce50d21a`** (26 nov, `738e660`). Incluye el popup al montar la tarjeta, la fila de fuentes por llegada y el registro de aterrizajes y referencias. `codesign --verify --deep --strict` pasó. La app, `nuun-engine` y `nu9ve` conservan los requisitos designados de Apple Development de la build anterior (`199f7b666f27`, respaldada en `app-backups/df3d96ea.app`). Pasaron 32 pruebas Swift con el motor recién compilado, 30 pruebas Rust enfocadas (5 diferenciales) y 9 de integración contra el motor firmado. Pendiente la aceptación de esta instalación en ventana. Las compilaciones locales usan `/Volumes/SUBSSD/001_builds/n-creator-utils/`. |
| Suites en macOS | Copia fijada de [nu9ve/core](../vendor/nu9ve-core/SOURCE) en **`249aeb1`** (26 nov: `nu9ve-time` sobre jiff mudado al core, `nu9ve-action-v1`, `Nu9veActionProgress`, `nu9ve-jobs`, `OneShotWorker`, `respond`). nüün borró `engine/crates/nu9ve-time` y usa el del core (`whole_seconds`, `micros`, `duration_since`). Python **422**, Rust **195** y **23** diferenciales con `--ignored` (sin las dos que se lanzan aparte), Swift **34**, dorado al día, fmt y Clippy limpios, frontera, `profile-examples.py --check` y `bench-examples.py --check` en verde, con `TZ=America/Phoenix` y `TMPDIR` físico. `tests/suite_lease.rs` necesita antes `cargo build -p nu9ve-mcp --bin nu9ve`. El gate incluye `sync-core.sh --check`; la ejecución en Windows queda en CI. |
| Tema de macOS | macOS 15+ y Swift 6.2+. La app usa [`Nu9veTheme`](../vendor/nu9ve-core/swift/README.md) y, desde el 26 nov (F3), **`NuMacShell`** del core: ventana cuadrada, barra de los tres comunes, Acerca de e identidad. Época «Con tu color» en Macintosh y Windows 9x (Ajustes → Apariencia). Swift pasa los 15 roles y el modo resuelto en los **110 casos** de los [vectores fijados](../vendor/nu9ve-core/data/theme-vectors-v1.json). El paquete se llama `Nuun` (antes `MediaMenu`, F2). Capturas `--qa-skins` antes y después en el scratchpad de la sesión; sin aceptar en ventana. |
| Motor en Windows | 16 nov, Windows 11 con Rust 1.99.1: compila, **141 pruebas** (15 omitidas: diferenciales y doradas, sin Python en el PC), fmt y clippy limpios. Pruebas C# del cliente **38 de 38** contra el binario real. [Compilación](#el-motor-vuelve-a-compilar-y-pasar-en-windows--2026-11-16) · [ingreso en la ventana](#ingreso-desde-la-ventana-de-windows--2026-11-16) |
| Instalar | `scripts/install_media_app.py`; se niega si la app está abierta (correcto: no se cierra por el usuario) |

## Por área

| Área | Estado | Evidencia física | Pendiente |
| --- | --- | --- | --- |
| **Identidad portable F0** | Motor Rust y Ajustes → Discos: marcador, segundo factor, índice y adopción en sólo lectura. | Ambos discos registrados y leídos con un estado local vacío, también en ventana. Índice resumido por proyecto: MEDIA9, 7 666 archivos en 27 grupos (20 KB, antes 5,1 MiB) y snapshot verificado; EDICION, 1 960 en 3 (2,7 KB). [Evidencia](#identidad-portable-f0--2026-11-22) | Otra Mac y Windows ([pasos](#pendientes-físicos-de-f0)). |
| **Ingreso de cámara** (Entrada → Flujo → Resultado) | En uso. Flujo identifica fuente y alcance individual/lote, y permite escribir el proyecto donde se prepara el plan. El detalle y los controles de un único resultado quedan visibles al lado o encima de la vista previa. | Varios ingresos reales a MEDIA9 + EDICION desde la app. La conversión Handycam y la expulsión se probaron en la build instalada. | Plan, progreso, pausa y reanudación comprobados en ventana con imágenes. Latencia del popup comprobada en la otra Mac ([PLAN](../PLAN.md#p0--uso-diario-sin-sorpresas)). |
| **Sony α6400** | En uso. Retira clips verificados, conserva índices y observa el formato al reconectar. Corregida la lectura final de entradas retiradas que siguen listadas | Job físico `17e70e69`: 21 archivos en MEDIA9 y 4 en EDICION cotejados con sus recibos por SHA-256. Doce retiros journalizados y nueve archivos de cámara intactos. [Detalle](#lectura-final-sony--2026-11-13) | La limpieza del clip real de prueba pasó con Rust. El retiro persistió tras remontar sin formatear. Falta observar el cierre por formato |
| **Stalker** | En uso: bandeja, bundles completos, liberación local con relink/ACK de Clipper | Ingresos reales completos | Casos `recording`/`finalizing`/`recovery` reales; [pedidos](contracts/README.md#pendientes-para-n-creator-utils) |
| **Dron / Atlas** | En uso de punta a punta: dispositivo por EXIF, registro de misiones, trabajo JPG, descartes y DNG a pedido. Atlas local lee `_missions/` y `units.ndjson`. La asociación de exports tardíos está disponible en el catálogo. Desde el 26 nov, el contenido de Ingreso avisa cuántas fotos esperan el export y si ya hay uno en Descargas, con «Incorporar y volver a leer» (sin aceptar en ventana). | 8 nov: SD `MAVIC200` respaldada, verificada y vaciada (job `cc8bd128`); EDICION mayo–junio respaldado (`77bdcb73`); DNG `dng-d9549817`. [Validación del lector Atlas](#dji-clipper-y-recuperación--2026-11-15) | Traslado journalizado de fotos ya archivadas y actualización de mapas. Confirmar un dispositivo nuevo desde la app. UI del plan de DNG. [Contrato](contracts/atlas-drone-v1.md#estado-del-lado-nüün--2026-11-08) |
| **A77 · exports de subaereo** | `missions_ingest_v1` en el socket local de macOS. Lee el buzón de salida de nove y reaprovecha A13. [Respuesta a nove](contracts/atlas-drone-v1.md#respuesta-de-nüün-a-a77-2026-11-26) | Socket y discos de fixture. Pasaron 192 pruebas de Rust, 23 diferenciales, fmt y Clippy, y 9 pruebas enfocadas de Python. Cubren reintento sin archivo o espejo, duplicado verificado y fuente intacta. 26 nov: el registro conserva aterrizajes (antes se perdían), Home por sesión, CSV a 1 Hz y referencias de VU10, también en `units.ndjson`, con prueba sintética y una copia del export real ([dónde lee Atlas](contracts/atlas-drone-v1.md#aterrizajes-home-telemetría-y-referencias-vu9-da12-vu10--2026-11-26)). | Entrega de punta a punta cuando nove implemente su cliente. |
| **Conversaciones en Inicio** | Tarjeta por SSD portable con destino y conteos por colección. Compara metadatos con el manifiesto y pide confirmar antes de actualizar la copia, aun sin MEDIA. La repetición sin cambios conserva el manifiesto. [Flujo](features/suite-disks-flows.md#3-sugerencias-al-conectar) | Discos de fixture: detección sin MEDIA, separación por dispositivo, rechazo de otro volumen, actualización incremental y repetición sin escrituras. Rust 194 pruebas y 23 diferenciales, fmt y Clippy. Swift 32 pruebas. Python 9 pruebas enfocadas de engine y suite. | Aceptar la tarjeta en ventana con la build instalada en la M4. |
| **Cierre de proyecto** | En uso: finales, publicados, regenerables de Clipper por su `cleanup-plan`, transcripciones a MEDIA | 8 nov: cierre conjunto de Terra aplicado (`8fe6e578`) | Seis verticales de Terra por publicar; aviso opcional por verticales viejos. [Contrato](contracts/project-closeout-v1.md) |
| **Perfiles de la suite y acumulados** | Motor Rust (`suite_profiles`, `suite_profiles_register`, `storage_accumulations*`), Ajustes → Perfiles → «Perfiles en esta Mac» y Discos → «Acumulados…». Fase 1: inventario y plan en sólo lectura | Fixtures y raíces aisladas. 26 nov: un escaneo real en sólo lectura de `~/code` desde la app (`20261126t064216z…`, 332 hallazgos, 257,7 GB; Rust `target/` 95,5 GB, cachés 65 GB, `node_modules` 25,4 GB). Ninguna corrida en `profiles.json` ni en la biblioteca real. [Detalle](#perfiles-y-acumulados--2026-11-25) | Registrar los perfiles reales (pedirlo), aceptar la vista en ventana, fase 2 (Papelera con recibo), copias en el bench, sesiones de agentes desde nove |
| **Biblioteca, Discos, cobertura** | Recuperación y entrega desde archivo. Bandeja por proyecto implementada en Swift con ubicaciones, entregas, Stalker y acceso al retiro de raws | Entrega desde MEDIA9 a EDICION (6 nov). [Diagnóstico de raws](#diagnóstico-de-edicion-y-música--2026-11-13) (13 nov) | Aceptar la bandeja en ventana. Fuentes SD asignadas. Ampliar cobertura al resto del EDICION; restaurar sin SD; rehash periódico |
| **Carpetas vivas** (música, samples, `0000_acomodar`) | M1 y M2 implementados por CLI/RPC | [Inventario SONIDO](features/music-sonido-inventory-2026-11-09.md) y [piloto de música](#diagnóstico-de-edicion-y-música--2026-11-13) | M1 requiere SONIDO, MEDIA7/501 y EDICION. M2: ampliar a producción de Ableton tras el piloto. [Fases](features/music-flows.md#fases) |
| **Fotos / Kyū, GoPro** | Detecta y planea con fixtures | Ninguna física | Aceptación por dispositivo |
| **Zoom H6** | Rust agrupa pistas y HPRJ, y usa la hora de inicio del WAV Broadcast. Los archivos sin clasificar quedan en la SD por defecto y se pueden seleccionar para respaldo. | 16 nov: nueva vista previa sobre `H6_SD` FAT32 con 7 sesiones, 28 miembros respaldables y 6 archivos Rekordbox retenidos. Sólo lectura de medios. | Confirmar el destino SONIDO y aceptar copia, verificación y retiro en hardware. |
| **Canon EOS Rebel SL3** | Rust lee modelo y reloj de JPG y de la miniatura EXIF interna del MP4. Conserva el índice CTG como soporte de archivo con el árbol original. [Naming](contracts/media-naming-v1.md#gramática) | 16 nov: descubrimiento y vista previa de 4 JPG, 2 MP4 y 1 CTG en la SD real `EOS_DIGITAL`, con planes en catálogo temporal | Copia y limpieza físicas, RAW+JPG y otros modelos aún pendientes. |
| **Sony Handycam DCR-SR42** | Rust convierte a MP4 con verificación y recibo de ambos hashes. El histórico conserva las fechas disponibles, omite coincidencias exactas y preserva variantes. Los dos modos entran en el lote y el HDD queda protegido contra limpieza por USB. [Contrato](contracts/ingest-flow-v1.md#hdd-de-handycam-por-usb) | 16 nov: el usuario completó y expulsó la Handycam desde la app. Job `84c0b834`, 14 miembros con recibo, SHA-256 del MP4 cotejado en MEDIA9 y SHA-256 del MPG registrado. Poner al día consolidó 1.093 MP4 (25.162.797.275 bytes), job `38289011`, y se cotejaron 673 coincidencias exactas. La versión corta de `M2U00995` permanece en edición. | Probar la conversión de cámara dentro de un lote mixto. Comprobar cobertura antes de formatear en cámara. |
| **DJI 360 / acción** | El planificador pide clasificar DJI cuando la evidencia es ambigua. En un fixture mixto, el clip asignado a 360 recibió rutas de archivo y edición de su proyecto mientras la foto del dron quedó pendiente de export | Ninguna física | Inspeccionar una tarjeta real por modelo y revisar los destinos en el plan |
| **Insta360 X3** | Rust agrupa los dos INSV y el proxy de cada toma, y conserva juntos los INSP/DNG con sus nombres originales. Ingreso propone sólo respaldo. La copia de edición se puede elegir por unidad. [Rutas](contracts/media-naming-v1.md#originales-insta360-en-rust) | 16 nov: tarjeta ExFAT real. Vista previa en catálogo temporal con 72 tomas y 4 pares de foto, sin misiones. Sólo lectura de originales X3. | Copiar una muestra y abrirla en Insta360 Studio. Sigue pendiente el par FreeFrame MP4/LRV. |
| **Recetas y Flujos** | La receta `nu9ve-default` describe familias de tarjeta (Stalker, Sony, H6, GoPro, Insta360, DJI, Canon) con su recomendación, tags de modelo, dispositivos por EXIF, categorías y flujos de misión. RPC/CLI `recipe` de sólo lectura; Flujos muestra «Lo que reconoce» desde ahí | Lectura del catálogo real con el motor instalado | Modelo por serial en vez de un tag por familia; editar la receta desde la app; generar los bloques de cada flujo desde la receta |
| **Distribución** | Sólo instalación local de desarrollo | — | Runtime empaquetado, Developer ID, notarización. [Gates](release.md) |
| **Motor en Rust** | En macOS, la app usa Rust para lecturas y escrituras compatibles, con Python disponible en Diagnóstico. En el sandbox, Rust declara **93 de 93 nombres de métodos** contando el índice de discos y `storage_collections`. Incluye las lecturas persistentes, el ingreso de tarjetas, las escrituras de configuración, planes y ejecución de liberación, Stalker y finales, entrega de trabajo, recuperación, restauración, respaldo de transcripciones y `coverage`. Windows tiene el ingreso de tarjetas por RPC. [Métodos y variantes](../engine/README.md#métodos) | Imágenes montadas FAT32, ExFAT y HFS+: siete escenarios de tarjetas comparados con Python. Sony conserva el índice y espera acción de cámara. GoPro, H6, Canon y DJI con misión terminaron la limpieza. DJI sin misión conservó la fuente. Prueba de cierre con un final en HFS+, y respaldo de catálogo comprobado por hash e integridad SQLite. | Aceptación en ventana con imágenes completada para Sony, Stalker, finales publicados y Discos. Copia y limpieza Sony aceptadas por RPC con MEDIA9 y la SD físicos. Extracción y reanudación física aceptadas el 13 nov. Pendiente semana de uso diario. [PLAN](../PLAN.md#motor-en-rust--ruta) |
| **Windows (cliente)** | Cliente WinUI 3 en [`native/windows`](../native/windows/README.md): shell con `TitleBar`, cinco skins, Inicio, Ingreso (Entrada → Flujo → Resultado), Flujos, Biblioteca, Ajustes, Acerca de, compacto/completo. Base reutilizable `Nu9ve.Core` + `Nu9ve.WinUI`, desde el 2026-11-16 en `nu9ve/core` y aquí como copia fijada; la identidad de tema de nüün vive en `Nuun.Core/Theme/NuunTheme.cs`. Setup con forma de SD. Con el motor de Rust muestra los datos reales del PC (`snapshot`, destinos, tarjetas montadas, contenido con dispositivo por EXIF), registra destinos y **hace el ingreso**: tarjeta u «Otra carpeta…», destinos reales, sólo respaldo confirmado por operación, `plan_document` → revisión → `apply` con progreso del motor → pausa cooperativa → `resume` (en Resultado y en Biblioteca → Actividad). `--maqueta` para demos y QA | 16 nov, Windows 11: 38 pruebas C# (dos de ingreso con el binario real). Recorrido por UI Automation en raíz de QA con carpetas locales: pausa y reanudación, y corte del trabajador con reanudación desde Actividad tras reabrir la app; 6 de 6 copias con el mismo SHA-256. [Detalle](#ingreso-desde-la-ventana-de-windows--2026-11-16). 9–10 nov: capturas de los cinco skins y registro de destino. MSIX **sin firmar**; Setup probado en demostración | Paso 4: SD y disco USB reales (exFAT, lector). Paso 5: expulsar, limpieza de cámara, progreso en la barra de tareas y aviso al terminar. Firma e instalación; alto contraste, escala y Narrador a mano. [Guía](../vendor/nu9ve-core/docs/platforms/windows.md) (su §1 todavía dice que nüün no lee discos) |

El motor usa `nu9ve-fs` desde su copia fijada para sincronizar, publicar y releer archivos. En macOS pasaron 161 pruebas de Rust y 23 diferenciales contra Python. [Contrato de archivos](../vendor/nu9ve-core/contracts/nu9ve-fs-v1.md).

## Identidad portable F0 · 2026-11-22

- Core fijado en `2d453339c711`. Se creó la identidad compartida de esta Mac
  con el nombre provisional del contrato y permisos `0600`. MEDIA9 quedó
  registrado como `archive` y EDICION como `work`; cada uno lleva
  `.nu9ve/volume.json` y `.nu9ve/apps/nuun/index.json`.
- Disk Arbitration y `diskutil info -plist` dieron en MEDIA9 el mismo
  `VolumeUUID` exFAT, `26B381C0-74B3-3F56-B1A3-35533249F725`. En una imagen
  exFAT desechable el UUID se mantuvo en dos montajes y cambió al modificar
  sólo el serial y sus checksums de arranque. Queda pendiente cotejar
  MEDIA9 en Windows, cuyo segundo factor usa el formato `XXXX-XXXX`.
- La adopción de los discos fue una operación física explícita con un estado
  local inexistente. Ambos índices pasaron los validadores de core y se
  resolvieron por `volume_id`. Se conservaron SHA-256 y fecha de modificación
  de marcadores, índices y del snapshot de MEDIA9 con su recibo. El snapshot
  es del 17 nov. MEDIA9 y EDICION reportaron 50 y 29 recibos, respectivamente.
  [Contrato](contracts/suite-volume-f0.md).
- Validación local con `unittest discover -s tests` (415 casos, 4 omitidos),
  `cargo test --workspace -- --include-ignored` (191), `cargo fmt --check`,
  Clippy con `-D warnings`, `swift test` (29), validadores de core y
  `sync-core.sh --check`. El primer intento de Python usó un temporal
  que rechazó el Stalker instalado. La corrida con el `TMPDIR` habitual
  resuelto pasó. El pin apunta al commit upstream de F0. El gate avisa
  de un commit local posterior de MIDI que todavía no está en el remoto.
- **Nombre del dispositivo** (2026-11-22): esta Mac es la **M1**. `device.json`
  pasó de «Mac 5577» a «M1» con el mismo `device_id` (`5577236d…`),
  `created_at` y `host_check`, permisos `0600`. Ajustes → Discos muestra y
  cambia el nombre. Lo lee con `suite_device` `peek`, que no crea el archivo:
  sin dispositivo dice que se crea al registrar el primer disco.
- **Aceptación en ventana** (2026-11-22, build de esta rama, entorno de
  `scripts/sandbox.py` con catálogo nuevo y `physical-volumes.json` que sólo
  admite MEDIA9 y EDICION por su `VolumeUUID`). Ajustes → Discos → «Adoptar
  este disco…», raíz elegida en el panel Abrir:
  - MEDIA9: «Disco adoptado en sólo lectura · 7 666 entradas · 23
    proyectos · Índice del 2026-11-23T00:56:50Z · Snapshot del catálogo
    verificado».
  - EDICION: «1 960 entradas · 3 proyectos · Índice del 2026-11-23T00:57:02Z ·
    Sin snapshot del catálogo».
  - SHA-256 y fechas de `.nu9ve/` de los dos discos y del snapshot de
    MEDIA9 con su recibo, iguales antes y después; ningún archivo nuevo en
    los discos ni `suite-volumes/` en el estado de prueba.
  - Por pulir: la fecha sale en ISO UTC y no en hora local; el panel no
    explica qué hacer después (importar al catálogo llega con F1).
- **Gate al cerrar** (2026-11-22, `c85b881`): `cargo fmt --check`, Clippy
  `-D warnings`, `cargo test` y `-- --ignored` con 1.99.1, `check-boundary.sh`,
  `swift test` (29), `engine_golden.py --check`, `sync-core.sh --check` y
  `--strict`. Python: 413 de 415; fallan los dos de `test_media_closure`
  porque el `clipper-core` local ya no porta `catalog_delivery`. No depende
  de F0 y en un checkout sin las CLIs de Clipper se omiten.
- **Tope del índice** (decisión del usuario, 2026-11-22): el índice de
  MEDIA9 pesa 5,1 MiB de 8 MiB (7 666 entradas de ~698 B, disco al 57 %).
  Se pasa a un dataset por proyecto, no por archivo; el detalle por archivo
  sigue en el snapshot. No cambia el contrato.
- **Índice resumido** (2026-11-22, versión 2 de `nuun_index`): un dataset por
  proyectos × carpeta de primer nivel, con la carpeta común de sus archivos,
  `size.items` y `size.bytes`, el peor estado y un `hash` `of: "manifest"`
  (SHA-256 de «ruta TAB sha256» ordenado) cuando todos están verificados. Un
  archivo en la raíz del disco sigue con su propio dataset. Publicado en los
  dos discos con copia previa de los índices viejos:
  - MEDIA9: 5 349 748 B → 20 408 B; 7 666 archivos en 27 grupos (23 `ok`,
    4 `changed`), 23 proyectos, snapshot verificado.
  - EDICION: 1 402 703 B → 2 675 B; 1 960 archivos en 3 grupos.
  - Los dos pasan `index_problem` de la referencia del core. Adoptados con
    estado aparte: `ok`, raíz `ok`, mismos conteos de archivos y proyectos.
  - La ventana de adopción muestra «N archivos en M grupos» y la fecha en hora
    local. Sin repetir la aceptación en ventana tras este cambio de texto.

- **`nu9ve-volumes` en el core** (2026-11-22, U2): el crate vive en
  [nu9ve/core `rust/nu9ve-volumes`](../vendor/nu9ve-core/rust/README.md#volúmenes),
  con el módulo `marker` (§1, §2, §3, §5 y §8 de `nu9ve-volume-v1`, probado
  contra los ejemplos del contrato con 1.99.1 y 1.89). nüün lo fija en
  `9711564`, borró `engine/crates/nu9ve-volumes` y `suite_volume.rs` usa esas
  reglas: el marcador rechaza ahora también un `previous_volume_id` igual al
  propio, como la referencia. `nu9ve-time` se queda aquí hasta que otra app lo
  necesite.

## Suite F1 con el agente · 2026-11-23

Seis pasos, un commit cada uno, todos con pruebas en carpetas de QA (`NU9VE_AGENT_DIR`, `NU9VE_DEVICE_FILE`,
`NU9VE_ENGINES_DIR`, `NU9VE_VOLUMES_DIR`, `HOME` temporal). No se tocó el agente instalado en la M1 (0.2.2,
`installed_by: installer`), su `device.json`, el llavero ni los LaunchAgents. Aceptado en ventana en el sandbox e
instalado (abajo).

1. **Core en `1d11d30`** (`ce7e781`): la copia suma `rust/nu9ve-mcp`, `nu9ve-ipc`, `nu9ve-device` y
   `nu9ve-exchange`. Gate: `sync-core.sh --check`, `check-boundary.sh`, cargo 161, Swift 29, Python 415, dorado.
2. **Agente en Ajustes** (`9930fd5`): el bundle lleva `Resources/backend/nu9ve` (compilado con
   `engine/Cargo.lock`, `cargo build -p nu9ve-mcp --bin nu9ve`). Al arrancar y al abrir Ajustes, `nu9ve agent
   state`; Ajustes → Suite muestra `absent`, `installed`, `running` u `outdated` y ofrece «Instalar el agente»,
   «Actualizar» (`agent install --by nuun`, sólo al pulsarlo) o «Arrancar» (`launchctl kickstart` del arranque
   instalado). Nunca `serve`. Prueba Swift con el binario real en QA: `absent` → instalar → `installed`, `keep`.
3. **`device.json` del agente** (`680554f`): con un `agent.json` válido (reglas de `nu9ve_mcp::agent`), nüün lee y
   renombra el mismo archivo pero no lo crea (`agent_device`) ni reemplaza uno copiado (`copied_device`). Sin agente
   lo sigue creando. Unitarias y `tests/suite_agent.rs` por `rpc`.
4. **Transporte registrable** (`0c21510`): mientras corre `rpc-serve`, un socket Unix en marcos con las reglas de
   motor §9 sirve `data_catalog` y `data_export`, registrado como `nuun` (`nuun-suite/1`). Bajo QA sólo con
   `NU9VE_ENGINES_DIR`. El catálogo sale del catálogo local y del último índice publicado de cada disco (ahora
   también en `suite-volumes/<volume_id>-index.json`), con raíces por `volume_id`, y de las colecciones vivas.
   `data_export` responde `too_large` con disco y ruta, o `not_found`: nunca bytes. Validadores del core:
   `data-examples.py --server` 11/11 y `export-examples.py --server` 8/8 (con `suite-frames`). A mano en QA,
   `nu9ve agent publish` escribió `catalog/nuun.json` leyendo este registro, y el registro se borró al cerrar.
5. **Colecciones por dispositivo** (`8ae733a`): una colección viva nueva va a `<carpeta>/<device_id>/…`; una que ya
   está en el catálogo conserva su carpeta. Sin identidad, no hay colección nueva. Python y el dorado sin cambios.
6. **«Trabajar aquí»** (`918a53f`): `suite_lease` llama `lease_take`, `lease_release` y `leases` del motor `suite`.
   En Proyectos, con el agente corriendo, el menú ofrece «Trabajar aquí» o «Dejar de trabajar aquí» y el resumen
   dice quién trabaja el proyecto. `tests/suite_lease.rs` con `nu9ve agent serve` real en QA.

**Aceptación en ventana** (2026-11-23, sandbox con el agente de QA dentro de su `HOME`; `45b47fd`):
- Ajustes → Suite: «The agent is not installed» → «Install the agent» → «installed but not running · 0.2.2»
  (`agent.json` de QA con `installed_by: nuun`; `~/Library/LaunchAgents` con el mismo hash antes y después; el
  agente de la M1 igual) → con `nu9ve agent serve` de QA, «The agent is running · 0.2.2».
- Discos: el nombre sale del `device.json` que creó el agente («Mac 54dc»); «Guardar nombre» lo pasó a «M-QA» con el
  mismo `device_id`, y `nu9ve agent status` lo vio.
- Proyectos → «prueba» → «Work here»: el resumen dice «You work here» y `leases` del agente tiene `here: true`;
  «Stop working here» lo quitó.
- `nu9ve agent publish` escribió `catalog/nuun.json` de la nüün abierta (1 proyecto; 0 datasets porque los discos
  de prueba no han publicado su índice).
- Corregido en la misma entrega: el sandbox no fijaba las variables de QA del agente (instalar habría registrado
  un LaunchAgent real), «Arrancar» aparecía con un agente de QA, y la app mandaba SIGTERM sin dar tiempo a
  `rpc-serve` de borrar su registro (ahora cierra stdin y espera hasta 0,5 s; comprobado: el registro se va).
- Observado: la clave de un proyecto es su id local en minúsculas, que en el catálogo es un UUID
  (`43788647-…`), no una clave legible como `baches`. Elegir claves de suite legibles queda para F1 (data §2).

Pendiente: Windows (sin named pipe, no se registra), la colección Agentes (`storage_collections`) en
`data_catalog`, el rechazo de un cliente de otro usuario (sin probar: hace falta otro usuario), aceptación en
ventana, y [notas para el core](#notas-para-la-sesión-del-core-f1).

## Sin datos personales en el repo · 2026-11-26

El código ya no trae perfil, país ni ciudad por omisión. `plan`, `save`, `save_capture_tag(s)` y la CLI toman lo que diga la petición y, si no, los `defaults` de la biblioteca. Si ninguno lo dice, el plan falla con `defaults_required`, igual en Python y en Rust (caso `no_defaults` del dorado). La lectura `defaults` responde `{}` en una biblioteca nueva, y la app arranca con perfil, país y ciudad vacíos hasta leer los guardados. El cierre ya no trata distinto a un perfil por su nombre: lo propio de cada perfil vive en `closure-profile:<id>`.

La configuración del importador antiguo tiene valores neutros, como [config.example.json](../config.example.json). Los discos, carpetas y nombres de tarjeta de cada persona van en `config.local.json`, ignorado por git, o en variables `NU9VE_*`. En la M1 se escribió ese archivo con los valores que antes estaban en el código, así que nada cambia ahí. La biblioteca real ya guardaba sus propios `defaults` y no tiene proyectos del perfil que tenía la regla especial; se leyó en sólo lectura y no se tocó.

Pruebas, fixtures y documentos usan nombres neutros: `canal-a`, `canal-b`, `blog`, `estudio-a`, «Ana», `oaxaca` y `TZ=America/Phoenix` (mismo desfase, sin horario de verano). Las rutas personales de los documentos pasaron a `~/`. El dorado se regeneró con Python 3.12. Cambiaron los parámetros de los planes y, por eso, sus `digest` e `id`; los planes son iguales.

**Evidencia.** Python 421 (una prueba nueva de los valores por omisión), Rust 217 con los diferenciales, Swift 32, `engine_golden.py --check`, `sync-core.sh --check`, `check-boundary.sh`, fmt, Clippy y `git diff --check`.

Los nombres de disco también son neutros en código, pruebas, maquetas de QA y documentos: `EDICION` (trabajo), `VIAJE` (segundo SSD de trabajo), `SONIDO` (audio) y `MEDIA7`, `MEDIA8` y `MEDIA9` (respaldos). Conservan el orden alfabético de los anteriores, y el dorado regenerado es idéntico salvo los nombres. Ninguna lógica dependía de ellos. La app ya no nombra un disco concreto: el flujo de audio dice «Disco de audio» y Ajustes, «el respaldo y el disco de trabajo».

**Queda.** La bitácora histórica nombra proyectos reales. El historial de git conserva lo anterior. Instalado como `3becf7b297ae`.

## Perfiles y acumulados · 2026-11-25

[Informe para el core y nove](suite-report-2026-11-25-profiles-accumulations.md). Core vuelto a fijar de `1d11d30` a `f267022` en un cambio propio ([`vendor/nu9ve-core.paths`](../vendor/nu9ve-core.paths)): suma `rust/nu9ve-profiles`, `rust/nu9ve-bench` y sus ayudantes y ejemplos de Python. Las versiones menores de `nu9ve-artifact-v1` (PDF y documentos) no tocan a nüün. La de `nu9ve-registry-v1` (`profile_id`) sí: el registro del motor `nuun` lleva ahora `library` (su catálogo) y `profile_id` (el perfil de omisión, sólo si ya tiene su entrada `nuun` en `profiles.json`).

1. **Perfiles en esta Mac** ([nu9ve-profile-v1](../vendor/nu9ve-core/contracts/nu9ve-profile-v1.md)). [`suite_profiles.rs`](../engine/crates/nuun-media/src/suite_profiles.rs) usa el crate del core. `suite_profiles` (lectura) lista perfiles, tipos, apps y bibliotecas, y qué perfiles creativos de nüün faltan. `suite_profiles_register` (escritura, pide `confirm: true`) agrega la entrada `nuun` con `upsert_app_entry`. Si el perfil ya existe (lo creó nove), sólo agrega su entrada y el nombre de nüün como alias (`estudio-a`). Crea los que faltan: `canal-a` como `person` y los demás como `team`. Nunca reescribe la entrada de otra app. La biblioteca de nüün es su catálogo, igual para todos sus perfiles. Bajo QA sólo se usa `NU9VE_PROFILES_FILE`. Pruebas: archivo inexistente, archivo de nove con `canal-a`, Rust y Python escribiendo a la vez, y un archivo inválido que no se toca ([test_suite_storage.py](../tests/test_suite_storage.py) y unitarias de Rust).
2. **Acumulados en disco**, fase 1 en sólo lectura ([diseño](features/storage-map.md#acumulados)). Las reglas son datos ([`rules.json`](../engine/crates/nuun-media/src/accumulations/rules.json)): Rust `target/`, `.next`/`.turbo`, `node_modules`, entornos y `__pycache__` de Python, SwiftPM y DerivedData, empaquetado, git, modelos, `~/Library/Caches`, Docker y respaldos de apps (`nove-update-backups/<fecha>`, `nu9ve/app-backups`). Cada hallazgo trae tamaño en disco, último uso, repo y su actividad, sesiones de agentes del proyecto, app y perfil dueños, si se regenera y su costo. El plan agrupa por tipo y pide dos confirmaciones para lo que no se regenera. Los discos se agrupan por volumen, con un aviso de casi lleno y un umbral del interno configurable. Las bibliotecas de `profiles.json` nunca son un hallazgo. CLI: [`scripts/suite_storage.py`](../scripts/suite_storage.py).
3. **Datos de la suite.** El escaneo más nuevo es el dataset `storage-accumulations` en `data_catalog`: `table`, `personal`, `copied`, con `revision` = escaneo y hash. `data_export` entrega su NDJSON (`ready`), una fila por hallazgo con `scanned_at`. Se guardan los últimos 26 escaneos en `<catálogo>/accumulations/`.
4. **Benchmarks** ([nu9ve-bench-v1](../vendor/nu9ve-core/contracts/nu9ve-bench-v1.md)). [`scripts/engine_bench.py`](../scripts/engine_bench.py) escribe en `bench/nuun.ndjson`, ignorado por git, con el ayudante del core. Mide seis lecturas del dorado en Python y en Rust, con `response_equal`, y el recorrido de un árbol de fixture: `storage_scan` en Python, unos 8 400 archivos/s, y acumulados en Rust, unos 146 000. El soak hace 2 000 lecturas del catálogo en un `rpc-serve` vivo: 0 B/iter con un límite de 4 096. Desde F6 corre en el gate ([`tests/test_engine_bench.py`](../tests/test_engine_bench.py)), y `engine_bench.py copy` mide la copia y su revisión. No usa `snapshot` porque pregunta por los discos montados.

5. **Exportar conversaciones a pedido de nove** (U1 de `nuun-conversations-compat.md` en nove). El socket de la suite anuncia `conversations_export_working_v1`. `conversations_export_working` arranca en segundo plano la copia de trabajo que la persona ya decidió en Ajustes → Conversaciones (el mismo disco y colecciones con `decided_at`; cualquier otra cosa es `confirmation_required`) y responde `run_id`. `conversations_export_status` dice cómo terminó. Sólo mientras la app tiene abierto su `rpc-serve`, como antes. El formato de la copia (`working.json`) no cambió: el contrato de compatibilidad sigue vigente tal como lo lee nove. La separación del sello de origen y de destino que propone sigue pendiente.

**No hecho:** copias en GiB/s en el bench (falta un caso de copia sobre fixture), la fase 2 (Papelera, recibo y medir lo liberado), leer las sesiones de agentes de nove (`agents:summary:sessions` por `suite_data_catalog`; hoy se usa el mtime de `~/.claude/projects/<ruta>`), U1 probado contra un nove real y aceptar la ventana. `check_localization.py` falla con 111 claves sin inglés. Ya fallaba antes; esta entrega suma las suyas. En la raíz de QA (`NU9VE_SANDBOX`) `HOME` es el de esa raíz; la app instalada usa el real.

## Discos de trabajo y conversaciones · 2026-11-23

[Diseño](features/suite-disks-flows.md) de los cuatro flujos pedidos (copiar entre discos de trabajo, respaldo de
conversaciones por dispositivo, sugerencias al conectar, otras Macs por el agente) y sus decisiones abiertas.

1. **Copias entre discos de trabajo** ([contrato](contracts/suite-work-copy-v1.md)): `volume_apps`, `work_copy_plan`,
   `work_copy_apply` y `work_copies` en el motor Rust, por `rpc`. Lee los índices de las demás apps del origen y
   guarda copias exactas de sus archivos en `.nu9ve/apps/nuun/copies/<id>/` del destino; recibo `nuun-work-copy/1` en
   los dos discos. Pruebas con discos de mentira (marcadores y seriales de fixture, `HOME` y catálogo temporales):
   copia con fechas conservadas, interrupción y reanudación, adopción de lo idéntico, conflicto, rol `archive`,
   rutas reservadas y disco cambiado entre revisión y copia. Sin ventana todavía; sin prueba física.
2. **Conversaciones por dispositivo** ([contrato](contracts/suite-conversations-v1.md)): `conversations_settings`
   (decisión explícita por colección y, aparte, para la copia de trabajo, con fecha), `conversations_status`,
   `conversations_backup_plan`/`_apply` y `conversations_run` (`now`, o `due` sólo en modo automático). Respaldo como
   colección viva con `include` (sólo historial; enlaces saltados y contados) en
   `004_datos/conversaciones/<colección>/<device_id>/`, recibo `nuun-conversations-backup/1` y copia de trabajo
   verificada `nuun-conversations-working/1`. Pruebas: 2 unitarias con un `HOME` de mentira (credenciales, ajustes,
   cachés y SQLite de Codex no salen; versión al crecer una sesión; nada a un `archive` como copia de trabajo).
3. **QA de punta a punta** (2026-11-23, raíz de QA propia con `NU9VE_*`, `HOME` y catálogo aislados; imágenes
   dispersas QA-MEDIA exFAT, QA-EDICION HFS+ y QA-VIAJE exFAT montadas): `suite_register_volume` en las dos de trabajo;
   `volume_apps` leyó el índice de nove; `work_copy_plan`/`_apply` copiaron 2 archivos con SHA-256 y fecha iguales,
   el recibo es el mismo en los dos discos y la copia exacta de `inplace-v1.json` coincide; EDICION sin
   `.nu9ve/apps/nove`. Respaldo de `claude-code` y `codex` a QA-MEDIA con su copia de trabajo en QA-EDICION; sin
   credenciales ni cachés; `due` pasó a falso. El sandbox de otra sesión (5 imágenes montadas) no se tocó.
4. **Sugerencias al conectar** (diseño §3): `suite_suggestions` (lectura) propone `work_copy` cuando una raíz de
   otra app en un disco de trabajo falta en el otro y ningún recibo la trajo, `conversations_backup` cuando una
   colección decidida está vencida y su MEDIA está conectado, y `conversations_working` cuando la copia de trabajo
   quedó atrás del último respaldo. Nunca en modo manual. `suggestions_dismiss` guarda «no volver a preguntar» por
   disco y flujo. 2 unitarias. Sin ventana todavía.
5. **Decisiones** (el usuario, 2026-11-23): las seis del diseño, como se propusieron.
6. **Ventana** (Swift): Inicio → «Discos conectados» muestra las sugerencias con «Ahora no» (hasta el siguiente disco
   que llegue) y «No volver a preguntar»; Discos → «Copiar al otro disco…» abre la hoja de copia (raíces de otras
   apps del origen, «Otra carpeta…», plan revisado, «Copiar y verificar», recibo); Ajustes → Conversaciones (disco de
   respaldo, colecciones con confirmación explícita, cuándo, copia de trabajo, «Respaldar ahora»). En modo automático
   la app respalda lo vencido al llegar un disco, con la app abierta (sin LaunchAgent). Swift 32/32; 62 cadenas
   nuevas en inglés (siguen faltando 56 de antes, de F0 y otras).
   **Aceptación en ventana** (2026-11-23, app de desarrollo en el sandbox con discos de mentira QA-MEDIA, QA-EDICION y
   QA-VIAJE y `HOME` de prueba): Inicio mostró las dos sugerencias; «Revisar copia…» → plan de 2 archivos (42 KB,
   proyecto subaereo, índices de nove) → «Copiar y verificar» → recibo en los dos discos, y la sugerencia
   desapareció. Ajustes → Conversaciones: activar Codex pidió confirmación; «Respaldar ahora» escribió
   `004_datos/conversaciones/{claude-code,codex}/<device_id>/` y quitó «Toca respaldar»; Inicio quedó «Todo al
   día». Corregido en la misma entrega: «Cerrar» de la hoja cerraba la ventana entera (usaba `dismiss` dentro de un
   `skinSheet`), y faltaba un indicador mientras se leen los índices. EDICION y MEDIA9 reales estaban conectados: el
   motor del sandbox los rechaza (`outside_sandbox`) antes de leer su marcador. No se instaló la app.
7. **Copiar proyecto y volver con lo procesado** (2026-11-23, para el [recorrido M1 → M4 →
   M1](../PLAN.md#recorrido-de-misiones-entre-la-m1-y-la-m4-pedido-del-usuario-2026-11-23)): la hoja lista los
   proyectos de nüün del índice publicado en el disco de origen; `work_copy_plan` acepta `update` (lo distinto se
   reemplaza tras verificar y la versión anterior queda en `.nu9ve/apps/nuun/copies/<id>/previous/`); la sugerencia
   `work_copy_changed` se calcula con los recibos. Prueba unitaria de ida y vuelta, con corte justo después de apartar
   la versión anterior. QA por `rpc` con QA-EDICION en HFS+ y QA-VIAJE en exFAT: proyecto «Baches» (2 carpetas, 3
   archivos) EDICION → VIAJE con fechas conservadas y sin el otro proyecto; tras «procesar» en el VIAJE (1 cambiado, 1
   nuevo) se sugirió la vuelta; sin `update` dio `destination_conflict`; con `update` reemplazó 1, copió 1 y adoptó 1;
   la versión anterior quedó apartada y no quedó sugerencia. En ventana: la tarjeta de vuelta → plan con «1 cambió y
   se actualiza» → copia verificada → Inicio «Todo al día». No se vio la lista de proyectos en ventana: el selector de
   disco es de otro proceso y no se puede manejar en segundo plano. Gate: Rust (152 en `nuun-media` y el resto),
   fmt, clippy, frontera, dorado y Swift 32/32. Sin prueba física.

### Notas para la sesión del core (F1)

- El motor `suite` de `nu9ve agent serve` no comprueba el uid del par en cada conexión (`agent/suite.rs`,
  `start`), como pide motor §9; el socket es `0600` en una carpeta `0700`, así que hoy no se nota.
- `ask_to_stop` busca el socket del agente dentro de su carpeta (`path.starts_with(dir)`), pero cuando la ruta es
  larga el socket va en `$TMPDIR/nu9ve-<hash>/suite.sock`: ahí `install` no le pediría parar por el motor.
- Intermitente previo, no de F1: `nuun-media integrations::tests::failures_stay_local_and_carry_pythons_messages`
  falló 1 de 8 corridas bajo carga (el Clipper falso agota sus 3 s).

### Pendientes físicos de F0

Los corre el usuario. Ninguno escribe en el disco salvo donde se dice.

**MEDIA9 en Windows** (cotejo del segundo factor, `nu9ve-volume-v1` §3 y §13.1):

1. En el PC, con el repo al día y el motor compilado (`cd engine && cargo build -p nuun-engine`, en Git Bash).
2. Conectar MEDIA9 y anotar la letra (p. ej. `E:`).
3. En PowerShell: `vol E:`. Anotar el número de serie, `XXXX-XXXX`.
4. En Git Bash, con un estado aparte para no tocar el catálogo del PC:
   `NU9VE_MEDIA_HOME="$TEMP/nuun-f0" NU9VE_DEVICE_FILE="$TEMP/nuun-f0/device.json" engine/target/debug/nuun-engine rpc <<< '{"id":1,"method":"suite_adopt_volume","params":{"root":"E:\"}}'`
5. Esperado: `state` `ok`, `dataset_count` 7 666, 23 proyectos, el snapshot `verified` y la raíz `disk` en `ok`. Como `fs.os` del marcador es `macos`, el serial de Windows no se compara con el de la Mac (§5.6).
6. Mismo comando con `"method":"suite_observe_volume"`: `current_fs.serial` debe ser lo que dio `vol`. Recuerda ese serial en `$TEMP/nuun-f0/suite-volumes/`, lo único que escribe y fuera del disco. Repetirlo debe dar `ok` otra vez.
7. Anotar aquí el serial, el estado y si el disco se montó con otro nombre.

**Otra Mac (M4)** (adopción en una máquina sin el catálogo de la M1). Nada escribe en los discos:

1. En la M4, `git pull` de `n-creator-utils` y build con `scripts/install_media_app.py` (o
   `scripts/sandbox.py open --app` para no tocar la nüün de la M4). El bundle trae `backend/nu9ve`.
2. Ajustes → Suite: anotar el estado del agente. Si dice «no está instalado», decidir:
   - **Sin agente** (F0 tal cual): Ajustes → Discos, escribir «M4» y «Guardar nombre». nüün crea
     `~/Library/Application Support/nu9ve/device.json` con un `device_id` distinto de `5577236d…`.
   - **Con agente**: «Instalar el agente» (instala `nu9ve` con su LaunchAgent `xyz.nu9ve.agent`). El agente crea
     `device.json`; nüün sólo lo nombra («M4» en Ajustes → Discos). Si el agente no corre, «Arrancar».
3. `cat ~/Library/Application\ Support/nu9ve/device.json`: `device_id` nuevo, `name` «M4», permisos `0600`.
4. Antes de adoptar: `shasum -a 256 /Volumes/MEDIA9/.nu9ve/volume.json /Volumes/MEDIA9/.nu9ve/apps/nuun/index.json`
   (y lo mismo en EDICION).
5. Conectar MEDIA9 y luego EDICION. Ajustes → Discos → «Adoptar este disco…» con cada raíz. Esperado:
   - MEDIA9: 7 666 archivos en 27 grupos, 23 proyectos, snapshot del catálogo verificado, estado `ok`;
   - EDICION: 1 960 archivos en 3 grupos, 3 proyectos, sin snapshot;
   - mismo sistema que el que registró y mismo serial, así que `ok` y no `suspect`.
6. Repetir el `shasum` del paso 4: debe ser igual.
7. Con el agente corriendo, opcional: `~/Library/Application\ Support/nu9ve/agent/nu9ve agent status` muestra la
   huella de la M4, que se compara con la de la M1 para emparejarlas (lo decide la persona).
8. Anotar aquí la versión de macOS, el estado del agente, el nombre con que montó cada disco y cualquier aviso.

**M4, paso 1 hecho (2026-11-24):** Mac mini M4, macOS 26.5.2. Build `85846cc19649` (la misma fuente que la M1) en
`~/Applications/nuun.app`, firmada con el certificado Apple Development del equipo y hardened runtime;
`codesign --verify --deep --strict` pasa, Gatekeeper la rechaza (esperado: no es Developer ID ni está notarizada).
Python externo de miniforge; Rust 1.99.1. Por falta de espacio en el disco interno, `engine/target/release` y
`.build/arm64-apple-macosx` apuntan a `/Volumes/SUBSSD/.dev-build/`. El VIAJE se reformateó como **SUBSSD en APFS**
(antes exFAT): sustituye al VIAJE en el [recorrido M1 ↔ M4](../PLAN.md#recorrido-de-misiones-entre-la-m1-y-la-m4-pedido-del-usuario-2026-11-23).

**Discos en la M4:** SUBSSD es el único destino en el catálogo local de la M4 (`work`, prefijo `001_proyectos`). EDICION y SUBSSD están montados con marcador `work`. El EDICION publica un índice de nüün con tres proyectos y se puede elegir en la hoja de copia. MEDIA sigue ausente. [Registro, copia y sugerencias](features/suite-disks-flows.md#permiso-registro-local-y-marcador-portable).

**Primera copia física Subaéreo:** el usuario completó en ventana EDICION → SUBSSD (`029dc7b0-57a3-4e2f-ab3a-30914a4f9069`). Recibos idénticos en ambos discos. Una lectura independiente de SUBSSD verificó los 2.702 archivos (2.616 JPG, 86 DNG) y el hash del manifiesto. Se copiaron 672 y se adoptaron 2.030. Alcance `001_proyectos/atlas-subaereo/misiones/2026/11`. Atlas apunta a SUBSSD. La primera selección incluía la carpeta mensual del índice. El mapa y el historial quedaron fuera de ese plan. La copia complementaria `4776d90d-da50-466a-b55b-ed1dd6366945` trasladó `units.ndjson`, `history.json` y `changes.ndjson`. Se verificaron los tres hashes y las tres versiones conservadas, y el lector de Atlas confirmó que los 2.616 JPG tienen mapa y captura en el historial del SUBSSD. El código actualizado añade los tres archivos al elegir la carpeta de proyecto en la hoja de copia. De 21 exports candidatos en Descargas, 4 hashes ya están en el registro de EDICION y 17 requieren cotejo al incorporar con MEDIA.

**Export físico de conversaciones sin MEDIA:** el motor nuevo exportó en la M4 a SUBSSD y copió a EDICION (`48c95987-2081-4e15-bb8a-525a1bb0377c`). Verificados en EDICION con lectura SHA-256 independiente contra los manifiestos: 1.941 archivos Claude Code, 573 Claude Desktop y 373 Codex, más tres manifiestos. Cero archivos cambiaron durante la lectura. Recibo idéntico en los dos discos. [Uso desde nüün y nove](features/suite-disks-flows.md#reunir-los-historiales-de-m4-y-m1-en-edicion).

**Build de desarrollo `a8b77aa47ce9`:** compilada y firmada con Apple Development en `dist/Nuun-m4-conversations-20261124-035131.app`, con el export a pedido y el progreso compartido en la hoja de copia. Verificación estricta de firma, compilación Swift release y seis pruebas enfocadas de Rust pasaron. El aviso de `rust-objcopy` por `libLLVM.dylib` ya aparece al compilar y no impidió producir los binarios. La app anterior sigue abierta. Pendiente instalar y validar las dos pantallas con datos de QA.


## Límites conocidos (vigentes)

1. El digest del plan no está autenticado: `apply` confía en verificaciones hechas al planear. Los jobs no tienen owner/lease, así que uno matado parece activo.
2. Los snapshots SQLite guardan decisiones humanas, pero no hay retención automática ni restauración completa desde la app. La recuperación por recibos no es un respaldo completo del catálogo.
3. Clipper: relink/ACK adoptados por CLI/TUI; faltan leases de consumidor y aceptación cruzada amplia. Final Cut queda fuera.
4. Sin firma Developer ID ni runtime portable en Mac. En Windows la ventana planea, respalda, pausa y reanuda con el motor de Rust, pero sólo se probó con carpetas locales en NTFS: nada con una tarjeta ni un disco USB reales, y nunca se liberó una tarjeta desde Windows. La relectura de verificación evita la caché (11 nov), probado sólo en NTFS. Linux no validado.
5. La CI de GitHub ([`checks.yml`](../.github/workflows/checks.yml)) comprueba la copia fijada, Python, el dorado, el motor en Mac y Windows y Swift, pero **sólo corre a mano** (`workflow_dispatch`, decisión del usuario del 2026-11-16): un push no la dispara. La evidencia de la tabla es local a macOS; falta confirmar el resultado de Windows en una corrida manual. Hay un `.gitlab-ci.yml` viejo (25 oct) sin Rust; no se usa ([B2](../vendor/nu9ve-core/docs/audits/fable-audit-2026-11-10.md#b2--no-hay-ci-y-el-diferencial-nunca-ha-corrido)).
6. La paridad Rust ↔ Python se mide desde el 11 nov (dorado y sombra), sólo para lo portado y sólo en lecturas sobre el catálogo real; las escrituras sólo en fixtures. Lo nuevo entra en Rust por decisión del usuario ([PLAN](../PLAN.md#motor-en-rust--ruta)).
7. ~~En un checkout limpio, `sync-core.sh --check` fallaba por tres fixtures del manifiesto que Git excluía con `bench/` en `.gitignore`~~ **Corregido el 2026-11-26:** la regla es ahora `/bench/` (sólo el registro local de `scripts/engine_bench.py` en la raíz) y `chart.csv`, `chart.png` y `atlas.ndjson` de la copia fijada quedan en git.

Esta página no autoriza operaciones sobre media real. Las pruebas automáticas usan fixtures temporales; cada corrida física es un plan revisado que el usuario pide.

## Entregas recientes

### El MCP de `media` en Rust · 2026-11-27

Paso (d) de F5 ([inventario](platforms/media-cli-callers.md#paso-d-hecho-el-2026-11-27)). `nuun-engine media mcp` sirve las ocho herramientas de sólo lectura de `media/mcp.py` por stdio ([mcp.rs](../engine/nuun-engine/src/mcp.rs)), y `nu9ve.py media mcp` le cede el proceso cuando hay un motor que lo entiende. [test_media_mcp_rust.py](../tests/test_media_mcp_rust.py): la misma conversación de 23 mensajes a los dos servidores, sobre dos catálogos del golden, con respuestas iguales (salvo los bytes libres del disco y `measured_at`, que cambian entre corridas), más la cesión desde `nu9ve.py` y la vuelta a Python con un motor viejo. Nadie configura este MCP hoy.

### Inglés completo en la app · 2026-11-27

`check_localization.py` pasa por primera vez: 1 236 claves, 0 sin inglés. Se agregaron las 112 que faltaban desde antes de F4 (Acumulados, Perfiles, identidad de discos, Handycam, adopción e índice portátil) al final de [en.lproj](../native/Nuun/Localization/en.lproj/Localizable.strings), con los mismos `%@` y `%lld` que la clave, comprobados por un script, y `plutil -lint` en verde. Las 22 que el script marca como sin uso se quedan: varias son textos del core (`PeriodWindowBar`) que llegan como `String` y el extractor no ve. Sin instalar: entra en la próxima build.

### `nu9ve.py media` responde con Rust · 2026-11-27

Paso (c) de F5 en [PLAN](../PLAN.md#tras-la-semana-de-uso-pedido-del-usuario-2026-11-26). Detalle en el [inventario](platforms/media-cli-callers.md#paso-c-hecho-el-2026-11-27).

- **Qué cambió:** las órdenes de lectura que mandan Clipper y Kyū (`projects`, `sources`, `volumes list`, `resolve`, más `captures`, `integration-capabilities` y `retitle-operations`) las responde `nuun-engine media`. Se elige `NUUN_ENGINE` si está puesto; si no, el motor junto a la app, el del checkout o el de la app instalada.
- **Cuándo sigue Python:** las demás órdenes, `NUUN_MEDIA_ENGINE=python`, sin motor, o cuando el motor sale con 2. Ese 2 llega con un error de uso o con un motor anterior a F5 (b): el de la app instalada hoy no tiene la CLI, así que con él responde Python.
- **Evidencia:** `ForwardTest` (4 pruebas nuevas) con un catálogo del golden y motores falsos. A mano, sin `NUUN_ENGINE`, la build `release` vieja salió con 2 y respondió la `debug` en 49 ms, contra 82 ms de Python.
- **Pendiente:** pasar a la sesión de Clipper el [aviso](platforms/media-cli-callers.md#aviso-para-la-sesión-de-clipper) y decidir cómo encuentran el motor sin el checkout. Las dos pruebas de `test_media_closure` que piden `item_analyze` fallan por el `clipper-core` instalado, igual que antes de este cambio.

### Vectores de nombres compartidos · 2026-11-27

Pedido de Stalker (`fdc80fc`) en [contratos](contracts/README.md#pendientes-para-n-creator-utils): el cambio de `_` en `tag` se copió a mano en Python, Rust y la Swift de Stalker.

- **El archivo:** [media-naming-vectors-v1.json](contracts/examples/media-naming-vectors-v1.json), entrada → salida de `slug`, `tag`, `duration`, `stamp` y el nombre de un bundle Stalker (zona `America/Phoenix`, colisión `-2[]` y `-3[]`). Incluye `_`, NFD, el máximo de 160 y los redondeos a par.
- **Quién lo prueba:** Python en [test_media_naming_vectors.py](../tests/test_media_naming_vectors.py) y Rust en `layout.rs` (`naming_vectors`; el nombre del bundle sólo con `TZ=America/Phoenix`, como el gate). `claim` de Python pasó a `layout.claim_name` para probarlo sin un plan.
- **Lo que encontró:** Rust no pasaba a NFC antes de limpiar, y un proyecto en NFD (como lo da una carpeta de HFS+) salía `N-andu` en vez de `Ñandú`. Arreglado con `unicode-normalization` 0.1.21, ya en la caché de cargo.
- **Para Stalker:** falta que copie los vectores. A primera lectura, su `slug` cuenta los 160 en grafemas y nüün en puntos de código tal como llegan: `ñ`×81 en NFD es error en nüün y nombre en Stalker.

### Claves de proyecto y Agentes en `data_catalog` · 2026-11-27

F7a y F7b de [PLAN](../PLAN.md#tras-la-semana-de-uso-pedido-del-usuario-2026-11-26). Están **sin instalar ni aceptar en ventana**.

- **F7a · Clave de suite legible** ([data §2](../vendor/nu9ve-core/contracts/nu9ve-data-v1.md#2-clave-de-proyecto-común), [project_keys.rs](../engine/crates/nuun-media/src/project_keys.rs)):
  - La persona la elige en Proyectos → menú → «Clave para la suite…». nüün sólo la ofrece: el título en minúsculas y sin acentos (`Baches de Ñuñoa` → `baches-de-nunoa`, con `-2` si ya está), o la clave conocida `conversaciones` para «Mis conversaciones». No empareja por su cuenta con otra app.
  - Hasta que se elija, la clave sigue siendo el id local, así que nada de lo que otra app ya vio cambia solo.
  - Una clave que tuvo un proyecto queda suya (`project-key-used:<clave>`) y no sirve para otro. Cambiarla se permite y, para las demás apps, es un proyecto nuevo.
  - `data_catalog`, el índice publicado en el disco y «Trabajar aquí» (las leases del agente) usan esa clave.
  - Métodos nuevos, sólo en Rust: `project_keys` (lectura) y `project_key_set` (escritura del catálogo).
- **F7b · La colección Agentes en `data_catalog`:**
  - Un dataset `agents:<agente>` por cada carpeta que existe (`~/.claude`, `~/.codex`, `~/Library/Application Support/Claude`), `files`, `personal` y en su sitio.
  - No se mide en cada llamada; `storage_collections` la mide cuando se pide.
  - Queda bajo `conversaciones` sólo si la persona dio esa clave a un proyecto de nüün.
  - Como todo lo `personal`, la suite ve su forma, no su contenido, salvo que la persona lo comparta.
- **Pruebas nuevas:**
  - En `project_keys`: la sugerencia, cuándo se elige, que una clave no se reutiliza, los errores y que `data_catalog` la usa.
  - `the_agents_folders_are_personal_datasets_in_place`, que corre con una carpeta de persona temporal: las pruebas no ven el `~/.claude` de esta Mac.
- **Pruebas:** Rust 211 y 23 diferenciales con `--ignored`; fmt y Clippy limpios, con `TMPDIR` físico y `TZ=America/Phoenix`. Swift 36, Python 426, golden al día y `sync-core.sh --check` sin error. `check_localization.py`: las 6 cadenas nuevas tienen inglés; quedan las mismas 112 claves que faltaban antes.

### Benchmarks de copia y soak en el gate · 2026-11-27

F6 de [PLAN](../PLAN.md#tras-la-semana-de-uso-pedido-del-usuario-2026-11-26), con [nu9ve-bench-v1](../vendor/nu9ve-core/contracts/nu9ve-bench-v1.md). Todo corre sobre fixtures en `TMPDIR`, nunca en discos de la persona.

- **Soak en el gate.** [`tests/test_engine_bench.py`](../tests/test_engine_bench.py) hace 400 lecturas del catálogo del golden a un `rpc-serve` vivo, en 0,3 s dentro de `python3 -m unittest discover -s tests`. Exige una corrida válida y `memory_slope_bytes_per_iter` bajo 4 096, lo mismo que `engine_bench.py soak --check-soak`. Medido: 68,7 B/iter y pico de 12,8 MiB.
- **Copia y revisión.** `engine_bench.py copy` compila con `--release` y corre `work_copy::tests::bench_work_copy`, que mide en discos de fixture. Escribe tres corridas al registro, válidas según `run_problem` del core. Medido en esta Mac, mediana de 3:

  | Caso | Tiempo | Ritmo |
  | --- | --- | --- |
  | `copy/work-copy`: copia verificada de 64 × 8 MiB | 6,8 s | **0,073 GiB/s** |
  | `review/work-copy-receipt`: revisión con recibo (U1.3) | **8,9 ms** | 7 177 archivos/s |
  | `review/work-copy-hash`: revisión sin recibo, como antes de U1.3 | 3,4 s | 19 archivos/s |

  Con recibo, la revisión es unas 380 veces más rápida. El caso `copy` no entra en los casos por omisión, porque tarda unos 40 s.
- **Hallazgo:** la copia verificada va a unos 75 MiB/s en el SSD interno. Escribe sin caché, sincroniza cada archivo y lo lee de vuelta. Puede que sea el costo de verificar, pero conviene medirlo en los discos reales antes de tocarlo.
- **Pruebas:** Rust 207 y 23 diferenciales con `--ignored`, sin las de medida; fmt y Clippy limpios, con `TMPDIR` físico y `TZ=America/Phoenix`. Python 426, con el soak. Golden al día y `sync-core.sh --check` sin error. No toca Swift.

### La CLI de media en Rust · 2026-11-27

F5 (b) de [PLAN](../PLAN.md#tras-la-semana-de-uso-pedido-del-usuario-2026-11-26). `nu9ve.py` no cambió, y Clipper y Kyū siguen usando Python hasta el paso (c).

- **Qué hace.** `nuun-engine [--state-dir D] media [--state-dir D] <orden>` responde las lecturas que mandan otras apps ([inventario](platforms/media-cli-callers.md#paso-b-hecho-el-2026-11-27)):
  - `projects`;
  - `sources --project P [--purpose edit|archive]`;
  - `volumes list`;
  - `resolve ASSET [--revision R]`;
  - `captures ROOT`;
  - `integration-capabilities`;
  - `retitle-operations`.
- **Sobre.** Es el de Python: `{ok, command, data}`, o `{ok: false, error}` con salida 1. Un error de uso sale con 2. Las claves salen ordenadas; los consumidores parsean el JSON, así que da igual.
- **Comprobado:**
  - `test_media_cli_rust.py` corre las órdenes con los dos motores sobre los catálogos `reads` y `projects` del golden y compara valores y salidas. Cada orden de Clipper y Kyū responde con datos al menos una vez.
  - En Rust, `media_orders_become_pythons_methods` prueba cómo se leen los argumentos.
- **Diferencia:** Python rechaza un `--state-dir` con un enlace simbólico en la ruta (`unsafe_state`) y Rust no.
- **Pruebas:** Rust 207 y 23 diferenciales con `--ignored`; fmt y Clippy limpios, con `TMPDIR` físico y `TZ=America/Phoenix`. Python 425 (3 nuevas), golden al día y `sync-core.sh --check` sin error. No toca Swift.

### Quién llama a `nu9ve.py media` · 2026-11-27

F5 (a) de [PLAN](../PLAN.md#tras-la-semana-de-uso-pedido-del-usuario-2026-11-26). Es sólo un inventario, en [media-cli-callers](platforms/media-cli-callers.md), y no cambia código.

- **Clipper**, en su motor de Rust y en Python, pide `projects`, `sources --project`, `volumes list` y `resolve`.
- **Kyū** pide `projects` y `sources --project [--purpose]`.
- **Stalker** ya habla con `nuun-engine rpc` desde la app instalada, y sólo en desarrollo cae a `nu9ve.py`.
- **`media mcp`** no lo tiene configurado ningún cliente en esta Mac.
- **nove, neuf y curado** no llaman a nüün.
- **`nu9ve` en el `PATH`** es sólo un alias de zsh, que otra app no ve: Clipper cae a la ruta del checkout y Kyū dice `nuun_unavailable` si no se configura su comando.
- **Rust** ya tiene los métodos; falta la CLI con argumentos, que es (b). `volumes` no está en las lecturas del golden.
- **Antes de (c)** hay que avisar a la sesión de Clipper. Además queda por decidir cómo encuentran Clipper y Kyū el motor: un `nu9ve` instalado que apunte al `nuun-engine` de la app, o buscar la app como hace Stalker.

### Los tres tamaños de una acción · 2026-11-27

U3 de [PLAN · UX](../PLAN.md#ux--acciones-y-procesos-pedido-del-usuario-2026-11-26), con las piezas de `Nu9veActionProgress`. Está **sin instalar ni aceptar en ventana**, y no se lanzaron capturas.

- **Pantalla (1):** la de F4c, en cada vista que lanza la acción.
- **Indicador (3):** el chip del core (`ActionProgressIndicator`) va en la barra de la ventana, completa y compacta. Reemplaza el *spinner* cuando lo que corre es una acción con registro y muestra el porcentaje de la última, o cuántas corren. El Dock muestra el porcentaje (`ActionDockBadge`).
- **Compacta (2):** un clic en el chip abre el panel del core (`ActionProgressCompact`), con una fila por acción y la más reciente arriba. Una fila lleva a la pantalla de esa acción, en la ventana completa:
  - el ingreso, a Ingreso;
  - las conversaciones, a Ajustes → Conversaciones;
  - el cierre y la liberación, a sus hojas;
  - la entrega, a Biblioteca → Archivos.

  Al salir de esa pantalla, la acción sigue en el panel y nunca se detiene. [Actions.swift](../native/Nuun/Sources/Nuun/Actions.swift).
- **Cierre.** Las acciones terminadas quedan en el panel con su cierre en una línea (listo, con avisos, no se pudo, cancelado) hasta «Limpiar terminadas». Si una termina mientras nüün no está al frente, llega un aviso del sistema con su resumen.
- **Sin hacer:**
  - «Reanudar», «Reintentar» y «Ver el recibo» en el cierre. La pantalla de cada vista sigue dejando paso a su resultado de antes.
  - Leer cada 5 s (`ActionViewSize.indicator`) cuando sólo se ve el chip.
  - El sonido de éxito.
- **Decisión abierta para el core:** el motor escribe `outcome.summary` y los avisos en inglés, como el resto de sus mensajes, y el cierre del core los muestra tal cual. Con la app en español se lee «Done: 2 files, 4.0 KB.». Hay dos salidas: que el contrato diga en qué idioma va `summary`, o que la vista arme la frase con números y `code` y deje `summary` para quien no sabe más.
- **Pruebas:** Swift 36 y `sync-core.sh --check` sin error. Esta fase no toca Rust ni Python, que pasaron completos en U1.3. `check_localization.py`: «Limpiar terminadas» tiene inglés, y siguen las mismas 112 claves que faltaban antes.

### Revisión de copia sin volver a leer · 2026-11-27

U1.3 de [PLAN · UX](../PLAN.md#ux--acciones-y-procesos-pedido-del-usuario-2026-11-26), **sin instalar ni aceptar en ventana**.

- **Qué cambia.** `work_copy::plan` ([work_copy.rs](../engine/crates/nuun-media/src/work_copy.rs)) ya no hashea los dos lados de cada archivo que está en el destino.
  - Si un recibo publicado entre esos dos discos lo verificó, en cualquier sentido, y los dos lados conservan su tamaño y fecha, es idéntico sin leerlo. El plan lo marca `verified_by`.
  - Lo demás se lee como antes. La copia vuelve a leer todo antes de adoptarlo.
  - [Contrato](contracts/suite-work-copy-v1.md#reglas).
- **Medida.** `review_of_a_finished_copy` (después `bench_work_copy`, F6; a mano, `--release`, con `TMPDIR` físico): revisar otra vez 64 archivos de 8 MiB ya copiados pasó de **3 489 ms** (mediana de 3) a **8–18 ms**, según la corrida. Falta llevar el caso a `nu9ve-bench` (F6).
- **La revisión es una acción.**
  - Primero lista y busca en los recibos; después lee sólo lo dudoso, con etapa `reviewing`, el doble de los bytes por leer y el archivo en mano.
  - En la app, «Revisar» manda `progress_id` y muestra la vista del core («Revisando la copia») en lugar del *loader*.
- **Arreglo en el registro (F4a).** El límite de escrituras ya no cede cada vez que termina un archivo: como pide el contrato, son como mucho 4 escrituras por segundo, más un cambio de etapa y lo retenido al terminar el paso. La primera medida de cada paso siempre se escribe. `several_plans_share_one_measure…` ahora comprueba eso en lugar de una escritura por archivo.
- **Pruebas:**
  - Nueva: `a_receipt_spares_reading_again_what_kept_its_size_and_time`, que cubre la ida, la vuelta, el mismo tamaño y fecha (que se confía sin leer) y otra fecha (que se lee y bloquea).
  - Rust 206 y 23 diferenciales con `--ignored`; fmt y Clippy limpios, con `TMPDIR` físico y `TZ=America/Phoenix`.
  - Swift 36, Python 422, golden al día y `sync-core.sh --check` sin error.
  - `check_localization.py`: las 2 cadenas nuevas tienen inglés; siguen las 112 claves que faltaban antes.
- **Decisión que queda a la vista:** un archivo editado que conserva su tamaño y su fecha se toma como idéntico en la revisión. La copia lo lee de vuelta y, si difiere, se detiene con `destination_conflict` sin tocarlo. Nunca se reemplaza en silencio.

### La app con la vista del core · 2026-11-27

F4c de [PLAN](../PLAN.md#tras-la-semana-de-uso-pedido-del-usuario-2026-11-26), **sin instalar ni aceptar en ventana**. La app ya no tiene vista de progreso propia: usa `Nu9veAction` y `Nu9veActionProgress` del core.

- **Lectura.** [`Actions.swift`](../native/Nuun/Sources/Nuun/Actions.swift) le da a cada lugar de la app su `ActionMonitor`, que pide `action_progress {progress_id}` al lector persistente. Hay cuatro lugares: la acción (liberar, entregar desde el respaldo, copiar entre discos), las conversaciones, el cierre y el ingreso. Desaparecen las tres lecturas propias (`action_progress {id}`, `closure_progress {plan_id}` y los `actionProgress`/`conversationProgress`/`closureProgress` del modelo).
- **Vista.** Las siete pantallas usan `ActionProgressScreen`, con el cierre del core (`ActionClosingView`) cuando el registro termina antes de que la llamada vuelva. Los colores llegan con `actionProgressStyle` desde la paleta. La tarjeta de Inicio de conversaciones usa la barra del core (`ActionLine`). Se borró `ActionProgress.swift`.
- **Ingreso.** `apply` y `resume` mandan `progress_id`. `job` sigue para la recuperación y los destinos. El tiempo restante es el del core, en lugar de `eta_seconds`.
- **Arreglos:**
  - La revisión del cierre usaba un UUID en mayúsculas como `review_id`. Desde F4a el motor lo rechaza con `invalid_params`; ahora va en minúsculas.
  - El último paso de un paso corto se perdía por el límite de 4 escrituras por segundo, y el cierre contaba «0 de nada». Ahora el paso escribe lo retenido al terminar (`Drop`).
- **Contrato estricto.** El motor quita el alias `id` y `action_progress` sólo acepta `progress_id`. `closure_progress {plan_id}` queda como alias del contrato. El registro del cierre ya no escribe `released_files`, sólo `nuun_released_files`. El oráculo de Python lee igual.
- **Del límite Rust–Swift.** `engine_records_fixture` (Rust) escribe los registros reales del motor en [un archivo](../native/Nuun/Tests/NuunCoreTests/Fixtures/engine-action-records.json): corriendo, completo con aviso, fallido, cancelado y cierre. Falla si se separan. `engineRecordsPassTheCoreReader` y `engineRecordsShowAsTheCoreViewExpects` (Swift) los leen con el lector del core.
- **Sin hacer todavía:**
  - El cierre queda visible sólo si llega antes de que vuelva la llamada. Mostrarlo con «Listo», «Reanudar» o «Reintentar» es de U3, con los tres tamaños.
  - Capturas: no se lanzó `--qa-snapshots` ni `--qa-skins`. Las capturas de QA ya alimentan el monitor con un registro del contrato.
- **Pruebas:** Rust 205 (nueva `engine_records_fixture`), más 23 diferenciales con `--ignored` (sin `real_export_copy_ingests_once` ni `cancel_file_child`); fmt y Clippy limpios, con `TMPDIR` físico y `TZ=America/Phoenix`. Swift 36 (dos nuevas), Python 422, `engine_golden.py --check` al día y `sync-core.sh --check` sin error. `check_localization.py`: el inglés de las 27 claves nuevas del core está en `en.lproj` y se quitaron las 5 que dejaron de usarse; quedan las mismas 112 que faltaban antes.

### Lectura `action_progress` del contrato · 2026-11-27

F4b de [PLAN](../PLAN.md#tras-la-semana-de-uso-pedido-del-usuario-2026-11-26), **sin instalar**. El motor ya sirve `action_progress` como dice [`nu9ve-action-v1` §2](../vendor/nu9ve-core/contracts/nu9ve-action-v1.md#2-método-action_progress).

- **El método.** [`action::progress_method`](../engine/crates/nuun-media/src/action.rs) recibe `{"progress_id"}` y lee `action-progress:<id>` y luego `closure-progress:<id>`. Si no hay registro, responde `{"contract":"nu9ve-action/1","progress_id":…,"state":"unknown"}` en lugar de `{}`. Sin `progress_id`, con otro parámetro, o con un tipo o patrón equivocado responde `invalid_params`.
- **Alias mientras la app migra.** `action_progress {"id"}` y `closure_progress {"plan_id"}` siguen valiendo, pero sólo solos. F4c quitó `id`.
- **`engine_info`** declara `capabilities: ["action_progress_v1"]`. nüün todavía no publica un archivo de [registro](../vendor/nu9ve-core/contracts/nu9ve-registry-v1.md), así que la capacidad sólo está en `engine_info`.
- **Python**, que es el oráculo, lee igual en `media/service.py`. Su productor no cambió y todavía borra el registro al terminar, así que ahí lo terminado responde `unknown`.
- **La app** compara con `{}` y ahora recibiría `unknown`. En esta fase no se instala: F4c cambia esas tres lecturas por `ActionMonitor`.
- **Pruebas:**
  - Nuevas: en Rust, `action_examples_v1` corre las 7 peticiones fijadas del core por `rpc-serve`, y `the_reader_serves_records_closures_and_unknown_ids` prueba el lector.
  - El golden suma 9 lecturas, con alias, caída al cierre y errores; Python y Rust responden igual.
  - Las dos pruebas de Python que esperaban `{}` ahora esperan `unknown`.
  - Rust 204, más 23 diferenciales con `--ignored` (sin `real_export_copy_ingests_once` ni `cancel_file_child`); fmt y Clippy limpios, con `TMPDIR` físico y `TZ=America/Phoenix`. Python 422, Swift 34, `engine_golden.py --check` al día y `sync-core.sh --check` sin error. `check_localization.py` sigue con las mismas 112 claves sin inglés de antes.

### Registro de acción `nu9ve-action-v1` en el motor · 2026-11-27

F4a de [PLAN](../PLAN.md#tras-la-semana-de-uso-pedido-del-usuario-2026-11-26), sólo en el motor de Rust, **sin instalar**: la app todavía lee con su `ActionProgressView` y sigue funcionando con los campos de antes. La vista del core llega en F4c.

- **El registro.** [`action.rs`](../engine/crates/nuun-media/src/action.rs) escribe `action-progress:<id>` (y `closure-progress:<id>` en el cierre). Lleva `contract`, `progress_id`, `action`, `state`, `unit`, `cancellable` y los tres instantes. Cada llamada al motor es una acción por `progress_id`: el primer paso abre el registro y los siguientes lo continúan sin que la barra retroceda (las tres colecciones de conversaciones, o la liberación tras el cierre). `action::within`, en el despachador, cierra cada acción con su respuesta: `complete` con `outcome` y avisos, `failed` con `error`, o `cancelled`, más `resumable` (falso en los planes, que se vuelven a revisar). El 100 % sólo llega con el cierre.
- **Latido.** Un hilo con su propia conexión mueve `updated_at` cada 5 s, también en los pasos que no informan (reenlace de Clipper, respaldo de metadatos). Se escribe como mucho 4 veces por segundo, y siempre al cambiar de etapa o terminar un archivo.
- **Se conserva.** `Drop` ya no borra: el registro terminado queda hasta que el mismo id vuelve a correr, y se retira al día siguiente. Un id que ya corre da `action_running`, salvo que su latido tenga más de 60 s. Un id que no cumple el patrón da `invalid_params`.
- **Etapas** comunes o `nuun.*` (`listing_source` → `listing`, `verifying_archive` → `verifying`, `relinking` → `nuun.relinking`…). Verbos: `copy`, `backup`, `release`, `closeout` e `ingest`. El cierre escribe también `nuun_released_files`; `released_files` queda hasta F4c.
- **Ingreso.** `apply` y `resume` aceptan `progress_id`: la transferencia de la tarjeta refleja su medida global (copiar, verificar y liberar) en el registro. `progress:<job>` sigue igual para la recuperación. La app todavía no lo manda.
- **Sin cambiar en F4a** (hecho en F4b, arriba): `action_progress {progress_id}`, `unknown` y `action_progress_v1`. Hasta F4c, `closure_progress` puede devolver el cierre de una corrida anterior del mismo plan.
- **Pruebas:** Rust 202, más las diferenciales con `--ignored` (sin `real_export_copy_ingests_once` ni `cancel_file_child`, que corre desde `a_cancel_file_pauses_the_copy`); fmt y Clippy limpios, con `TMPDIR` físico y `TZ=America/Phoenix`. Python 422, Swift 34, `engine_golden.py --check` al día. Nuevas: seis en `action::tests`; las de cierre, conversaciones, transcripciones, entrega y cancelación del ingreso ahora comprueban el registro cerrado en lugar de su ausencia. `check_localization.py` sigue fallando con 112 claves sin inglés, como antes (esta fase no toca Swift). `sync-core.sh --check` avisa que el core avanzó después de `6718418` (documentos y Windows); no se volvió a fijar.

### Sombra sobre el catálogo real · 2026-11-26

`scripts/engine_shadow.py replay` con el motor instalado (`c0de130ed8e9`), sólo lectura, tras el respaldo de conversaciones: 40 métodos y 243 lecturas. Todas iguales a Python (`closure_link_candidates`: 6 iguales y 3 con el mismo error), salvo `storage_collections`: difiere en `measured_at` y, en la primera vuelta, en los bytes de la colección de agentes (`~/.claude` crecía con sesiones abiertas). Rust fue más rápido o igual en las lecturas lentas: `snapshot` 706 ms contra 2 028, `source_cards` 416 contra 1 454, arranque 6,6 ms contra 82. La sombra en vivo (`enable`/`report`) ya no registra: la app pregunta primero a Rust y Python sólo responde lo que Rust no tiene; el último registro es del 16 nov. Con esto y el uso desde el 19 nov, el criterio de la semana queda cumplido salvo la aceptación en ventana pendiente ([PLAN](../PLAN.md#p0--uso-diario-sin-sorpresas)).

### Acciones y sugerencias · 2026-11-26

U1.1, U1.2 y U1.4 de [PLAN · UX](../PLAN.md#ux--acciones-y-procesos-pedido-del-usuario-2026-11-26), en código y **sin instalar ni aceptar en ventana**.

- **Respaldo de conversaciones con progreso.** `conversations_run` acepta `progress_id`: el listado de cada colección y la copia de las tres se ven en una sola medida (`living::apply` suma desde donde terminó la anterior con `progress_offset`/`progress_total`). Ajustes → Conversaciones muestra `ActionProgressView` mientras corre y la tarjeta de Inicio una línea con barra y bytes. Antes: 37 min con un *spinner* para 7 403 archivos y 4,4 GB a MEDIA502 (recibos `1ca44cef`, `9dfc2086`, `4e06c3d7`; 17 archivos cambiaron durante la copia y quedan para el próximo).
- **Sugerencias sin ida y vuelta.** `changed_since_copy` ya no propone lo que el otro disco tiene igual (tamaño y fecha). Inicio junta en una tarjeta las dos direcciones de un par de discos y las conversaciones por copiar a varios discos, con un botón por dirección o disco.
- **Terminal:** las dos líneas de la travesía se centran cada una.
- Pruebas: Rust 201 (más 16 diferenciales con `--ignored`; fallan sueltas, como siempre, `real_export_copy_ingests_once` sin `NUUN_MISSIONS_EXPORT` y `cancel_file_child`, que lanza otra prueba), fmt y Clippy limpios, con `TMPDIR` físico y `TZ=America/Phoenix`; Swift 34. Nuevas: `several_plans_share_one_measure_from_where_the_previous_ended` y `what_both_disks_already_hold_alike_is_not_proposed_back`. Sin `TMPDIR` físico, `writes_go_only_through_the_one_shot_rpc` da `unsafe_state` (entorno, no regresión).


### Colección Agentes · 2026-11-19

**Qué se hizo.** `storage_collections` mide `~/.claude`, `~/.codex` y `~/Library/Application Support/Claude` sólo con metadatos, en Python y en Rust, y lo expone por RPC y MCP. Reparte el espacio en historial, estado, caché regenerable, registros y otros, y estima cuánto crece por semana. [Diseño](features/storage-map.md#colecciones).

**Lectura real en esta Mac**, con permiso del usuario y sólo tamaños y fechas. Python tardó 1.4 s y Rust 0.9 s, con el mismo JSON en las tres raíces.
- **Total:** 8.42 GiB asignados en 102,248 archivos.
- **Por raíz:** Claude Code 1.28 GiB, Codex 5.15 GiB y la app de Claude 1.99 GiB.
- **Por clase:** historial 5.20 GiB, caché regenerable 2.21 GiB, estado 520 MiB, registros 95 MiB y sin clasificar 417 MiB.
- **Lo más grande:** en Codex, `thread_history_1.sqlite` 1.62 GiB, `sessions` 1.55 GiB y `archived_sessions` 403 MiB. En Claude Code, `projects` 1.19 GiB. En la app, `Cache` 921 MiB, `claude-code` 675 MiB y `Code Cache` 389 MiB.
- **Crecimiento, por fecha de creación:** el historial crece unos **232 MiB por semana** (promedio de las 4 semanas completas hasta la del 9 nov). El total da 803 MiB por semana, inflado porque las cachés de la app se recrearon en la semana del 9 nov (920 MiB).

**Evidencia.** [`tests/test_agent_storage.py`](../tests/test_agent_storage.py), 6 pruebas con un `HOME` de fixture, incluida la paridad de Rust por `rpc-serve`. Además pasaron la suite de Python completa (415), el workspace de Rust con `TMPDIR` físico y `TZ=America/Phoenix` (166 pruebas, 2 nuevas, y las ignoradas de siempre), `fmt` y `clippy -D warnings`. Con el `TMPDIR` por defecto, que es un enlace en `/var`, fallan 11 pruebas de Rust ya existentes. Fallan igual en `f18d6bd`, así que no son de este cambio.

**Pendiente.** Acceso desde el sandbox de la app, el plan revisado para retirar cachés y `nu9ve-data-v1`. Están en el [PLAN](../PLAN.md#p2--siguientes-flujos-de-producto).

### Ingreso desde la ventana de Windows · 2026-11-16

Fase A, paso 3 del [PLAN](../PLAN.md#motor-en-rust--ruta). (1) **Motor:** pausa cooperativa en Windows con un evento con nombre (`NUUN_CANCEL_EVENT`, [interrupt.rs](../engine/crates/nuun-media/src/interrupt.rs)) en los mismos puntos recuperables que SIGINT en Mac; `windows-sys` gana la función `Win32_System_Threading` (misma versión, sin descargas). macOS no cambia de comportamiento, pero no se recompiló allá. (2) **Cliente:** `RpcEngine` planea con `plan_document` y aplica o reanuda en un trabajador `rpc` por mutación, con el documento y el digest revisados; el progreso sale de `job` (`overall_done`/`overall_total`). Se quitó `not_on_windows`. (3) **Ventana:** «Otra carpeta…» (selector o ruta), destinos registrados en línea, confirmación de sólo respaldo que se reinicia por operación, liberar sólo para la raíz de una tarjeta, Pausar, y Reanudar en Resultado y en Biblioteca → Actividad. Con datos reales el proyecto ya no viene como «proyecto-demo». Evidencia: `RustEngineIngestTests` (carpeta con sólo respaldo; cancelar a media copia → `paused` → `resume`); recorrido por UI Automation en raíz de QA con 3 × 128 MB: pausa con 1 archivo verificado y reanudación, y luego segunda carpeta con el trabajador matado a media copia, «Se detuvo: nada se limpió sin verificar», app cerrada y reabierta, «Reanudar» en Actividad hasta «Listo»; 6 originales y 6 copias con el mismo SHA-256 y ningún `.ncpartial`. Gate: Rust 141 pruebas, fmt y clippy; C# 38 de 38; captura de la maqueta sin cambios. Pendiente: paso 4 (SD y disco USB reales) y paso 5.

### El motor vuelve a compilar y pasar en Windows · 2026-11-16

Desde el 11 nov el motor sólo se había compilado en la Mac. Al retomar la fase A en el PC hicieron falta 18 crates fijadas en `Cargo.lock` que no estaban en la caché (`jiff`, `rustix`, `iana-time-zone`…; se descargaron con permiso del usuario, sin cambiar versiones). Aparecieron cinco fallos sólo de Windows, ya corregidos: (1) **del motor**: el final movido a publicados devolvía `current_path` con `\` ([projects.rs](../engine/crates/nuun-media/src/projects.rs)); ahora usa `/` como Python y el resto del catálogo; (2) el ayudante `count` de las pruebas de `transfer` comparaba con `/`; (3–4) clippy: `Guard::acquire_at` e `Integrations::fixture` sólo los usan pruebas de Unix y pasan a `cfg(all(test, unix))`; (5) la prueba C# de `snapshot` esperaba `capabilities` sin portar (se portó el 12 nov). Evidencia: `cargo test`, `fmt --check`, `clippy --all-targets -D warnings` y `dotnet test native\windows\Nuun.Core.Tests` en Windows 11. Sin probar en la Mac tras el cambio: el `/` es el mismo resultado que antes allí.

### nu9ve-engine-v1 en los dos motores · 2026-11-16

Sólo el protocolo de `rpc-serve`/`rpc` y la sombra; la app no se reinstaló. (1) **Brechas de la [§12.1](../vendor/nu9ve-core/contracts/nu9ve-engine-v1.md#121-nüün):** un método que ningún motor tiene es `unknown_method` antes que `read_only` ([`METHODS`](../media/service.py) es el catálogo de Python, que Rust copia en `ORACLE_METHODS`); el `id` se lee antes que `method` y sólo vale entero, cadena o `null`; un entero de más de 64 bits vuelve con sus dígitos en Rust (`raw_value`, sin `arbitrary_precision`); `params` que no es objeto es `invalid_params` y JSON roto, `invalid_request`; Python responde `engine_info` y los dos dicen `contract`, `protocol` (`nuun-media/1`), `capabilities` y `native`. (2) `operation_failed` pasa a **`internal_error`** en Python y Rust a la vez; ningún cliente Swift o C# lo leía. (3) **Sombra:** compara tipos (`1` no es `1.0`, `true` no es `1`) y el registro guarda sólo rutas, códigos, `reason` y tiempos, nunca valores ni mensajes. Evidencia: los **20 casos de motor** de los [ejemplos fijados](../vendor/nu9ve-core/data/engine-examples-v1.json) pasan en Python y Rust ([tests/test_engine_examples.py](../tests/test_engine_examples.py), `engine_examples_v1` en Rust y el cliente `scripts/engine-examples.py` de core), y los 10 de sombra en [tests/test_engine_shadow.py](../tests/test_engine_shadow.py). Gate: Rust 1.99.1 fmt, clippy y pruebas; 23 diferenciales con `TZ=America/Phoenix`; `engine_golden.py --check` (regenerado: sólo cambian los dos códigos); unittest; `swift test`; `check_suite_boundary.sh`. `replay` estricto sobre el catálogo del dorado (no el real): ninguna diferencia nueva por la comparación de tipos. Las dos que da ya estaban con la laxa y son del port: `job` sobre un plan sin `files` (Python falla con `KeyError`, `internal_error`; Rust responde) y `storage_overview` `fs_free`/`fs_used` (espacio libre leído en vivo, cambia entre las dos lecturas).

El transporte de [`nu9ve-engine`](../vendor/nu9ve-core/rust/nu9ve-engine/Cargo.toml) ya sirve `rpc-serve` en Rust. Cerrados los puntos restantes de la §12.1: `not_ported` lleva `reason`, ambos servidores descartan líneas de más con memoria acotada y `rpc` de Python devuelve `id` y conserva `command` en éxitos. Los dos motores exigen que `params` sea un objeto o se omita, y que para `engine_info` esté vacío. En ambos modos se comprobó un id de 350 dígitos. El árbol de features de Rust activa `raw_value` sin `arbitrary_precision`.

La repetición de [`engine_shadow.py replay`](../scripts/engine_shadow.py) sobre un catálogo temporal cargado con `reads.sql` del dorado dio los mismos 162 resultados antes y después de adoptar el crate: 138 `same`, 18 `same_error` y seis `error_mismatch` para dos jobs sin `files` (tres repeticiones cada uno). La validación del binario de Windows queda en CI.

### DJI, Clipper y recuperación · 2026-11-15

El lote incorpora exports y permite recalcular una SD con sus asignaciones originales. Compara pendientes y rutas antes de sustituir el plan. Fotos ya archivadas pueden asociarse a misiones en el catálogo tras rehash, conservando sus rutas. Proyectos puede preparar y restaurar una copia en la ruta exacta de Clipper, comprobar la referencia después y mostrar IDs de cola. [Lote](features/save-all.md) · [Dron](contracts/atlas-drone-v1.md) · [Clipper](contracts/project-closeout-v1.md#restaurar-una-ruta-que-clipper-sigue-usando).

Recuperación de sólo lectura con MEDIA9 y sin SD: el snapshot `5124be72` coincidió con SHA-256 del recibo y pasó `integrity_check` en un estado temporal. Rust leyó 20 proyectos, 2 perfiles y 2 decisiones de cierre. La copia contiene 20 aliases, 3189 estados de trabajo y 8579 recibos de copia. No se sustituyó el catálogo diario. Atlas local ya consume el mapa y el registro. Su lector abrió 2985 rutas JPG de EDICION, todas presentes, y resolvió los 2451 IDs de misión contra el registro; 13 pruebas de su catálogo pasaron. Quedan ensayos físicos H6, GoPro, DJI 360/acción y RAW+JPG.

Build de desarrollo `f7c3f4f012ab` instalada y abierta. Pasaron 150 pruebas Rust, con diferenciales, y 28 Swift. El motor empaquetado preparó, en sólo lectura, cinco restauraciones exactas de Clipper (26 miembros, 12.135.502.496 bytes) y el informe de MEDIA9 devolvió cero unidades pendientes de asociación. Las restauraciones quedaron como planes.

### Lote recuperable y estado de Clipper · 2026-11-15

«Retomar lote» conserva los planes pendientes con reemplazo atómico y muestra los jobs iniciados en Actividad. Conserva un solo resumen local. Rust admite copias de edición por proyecto y carpetas vivas registradas; los respaldos incrementales completados desde el lote reducen su plan a un recibo compacto. Un cambio del registro DJI exige volver a revisar una tarjeta cuyo job aún no inició. [Contrato](features/save-all.md).

Evidencia: 28 pruebas Swift, 129 pruebas Rust de binario y motor y 19 diferenciales. Clippy limpio. Fixtures de reapertura y compactación, entrega de trabajo desde recibos, copia de carpeta viva y registro DJI cambiado. Se inspeccionó el selector en la build empaquetada usando fixtures. Falta recorrido completo de ventana con interrupción y reconexión de dispositivos.

Auditoría de sólo lectura de EDICION, catálogo y Clipper: cinco rutas de edición ausentes siguen en uso en dos proyectos y las cinco tienen ubicación visible en MEDIA9. Siete assets de un proyecto de dron siguen ligados a la cola. [Observación por proyecto y límites](contracts/project-closeout-v1.md#observación-en-edicion--2026-11-15).

### Poner al día · 2026-11-14

Inicio completo muestra espacio disponible y el acceso a pendientes. Compacto y selección tienen el acceso en la barra. «Agregar al lote» reúne planes preparados de SD, Stalker y finales publicados. Rust valida los documentos, identidades, destinos y capacidad antes de ejecutar; conserva el resultado y el journal de cada operación iniciada. Misiones, copias pendientes, cierres, liberaciones y jobs con seguimiento abren sus revisiones existentes. La selección del lote vive en la sesión. [Flujos, contrato y límites](features/save-all.md).

Evidencia con fixtures: 28 pruebas Swift, 7 del binario Rust y 118 del motor (diferenciales ignorados en esta corrida); Clippy limpio. El caso de lote rechazó superposición de fuentes, espacio insuficiente y orden distinto al revisado, y ejecutó dos planes con políticas de origen distintas. QA visual aislada de Inicio, barra compacta y selector. Build de desarrollo `d5ead0609529` instalada y firma verificada. Queda recorrer selección, revisión y resultado en ventana.

### Diagnóstico de EDICION y música · 2026-11-13

Con `nuun-engine` instalado y una copia temporal del catálogo, `coverage --no-hash` encontró coincidencia de tamaño para 88 archivos (32.528.540.349 bytes) de `001_proyectos/canal-a_raw`. Cuatro revisiones verificaron SHA-256 en MEDIA9. Debloateando mi tele (8,14 GB) no requirió reenlaces. Buscando baches con el dron (9,60 GB) y El repositorio de nu9ve (6,75 GB) requirieron cuatro y dos reenlaces, respectivamente. Buscando baches en Oaxaca con el dron (7,86 GB) quedó bloqueado por exportaciones y snapshots de Clipper. Los cuatro cierres editoriales todavía requerían vincular entregas y declararlas completas. El plan independiente de raws evalúa sus propias dependencias.

Con autorización, Rust aplicó el plan de Debloateando mi tele `b5c9738a` en el catálogo diario. Confirmados los 28 retiros (8.135.159.777 bytes), la presencia de sus 28 copias de archivo y 41 GiB libres en EDICION al terminar. El plan cubrió únicamente su carpeta raw.

Rust completó el piloto de carpeta viva `1845c81a`, de `~/Music/03_final` a `MEDIA9/005_produccion/music-03_final`: 491 archivos y 9.836.439.290 bytes verificados. El manifiesto de archivo contiene los 491 y el segundo plan devolvió cero nuevos, cero cambiados y 491 sin cambios. El informe de referencias absolutas encontró un set y cero rutas externas. El XML de Live 10 de `parasito.als` contiene 15 muestras únicas referenciadas con rutas relativas. Se leyó el set de la copia y se verificaron su SHA-256 y los de las 15 muestras contra el manifiesto. Queda abrirlo en Ableton Live.

La bandeja compiló en la build ad-hoc `f238fda455c3`; pasaron 28 pruebas Swift, la QA aislada de asignación y renombre de proyectos Stalker y tres pruebas enfocadas de música en Rust (diferencial con Python, protección y reanudación de M1, versiones y reanudación de M2). La build se instaló y se comprobó el arranque de la app y su motor. Queda la revisión en ventana de la bandeja nueva.

### Misiones DJI al incorporar un export · 2026-11-13

En `MAVIC200`, al reproducir el registro desde los exports comprimidos y comprobar sus SHA-256, el export de 24 h dejaba **966 fotos con misión y 209 en espera**, todas del 9 nov. `coverage-7d-20261113-135240.json` resolvió las 209. `discover` dio la misma respuesta en Rust y Python: **1.175 pares JPG+DNG con 1.175 capturas distintas**, más dos archivos auxiliares de cámara. `plan_document` de Rust, con una copia local del catálogo, MEDIA9 y EDICION, mostró **1.175 de misión, 0 sin misión y 0 en espera**. No se aplicó ese plan ni se modificó la tarjeta.

La ventana ahora vuelve a leer la entrada tras incorporar el export, invalida los planes anteriores de ese archivo y conserva las asignaciones manuales. Pasaron las 28 pruebas Swift y el caso `--qa-mission-import` con dos pares sintéticos: de espera a misión, plan anterior invalidado y plan nuevo con dos coincidencias. Esto actúa sobre la revisión de la fuente; reclasificar fotos que ya se archivaron sigue pendiente.

### Reanudación física Sony con Rust · 2026-11-13

Se extrajo la SD del ensayo con 4 MiB del clip en un parcial del job `a91ccb13`. Rust registró `OSError` y el diario quedó con seis miembros verificados y cero retiros. La primera reanudación, con el mismo UUID, se detuvo: macOS había reasignado el inode del marcador Sony vacío `SONYCARD.IND`. La corrección admite sólo esa variación en el marcador de tamaño cero, conserva los demás atributos y relee por fuera de la caché las copias verificadas del intento anterior.

Se reanudó el mismo trabajo y digest. Rust verificó 903.115.583 bytes en 12 archivos de MEDIA9, guardó el recibo y retiró sólo el clip, el XML y su miniatura. Los nueve archivos de cámara conservaron sus hashes y no hubo parciales. La build `e70e525ae792` volvió a cotejar el recibo e informó `waiting_camera`. Pasaron 176 pruebas Rust con diferenciales, 28 Swift, clippy y el dorado.

### Ingreso físico Sony con Rust · 2026-11-13

Con autorización por operación se limitó el backend a la SD Sony y a `MEDIA9/nuun-qa-20261113` por ruta y UUID, con catálogo temporal. El job sintético `300eaca4` verificó el archivo en ExFAT y conservó el origen. El job `ac2884b4` verificó 12 archivos en MEDIA9 por SHA-256 y cotejó el recibo con el digest del plan. Retiró únicamente clip, XML y miniatura. Los nueve archivos de cámara conservaron sus hashes. Sin parciales ni degradación de la lectura de verificación declarada. El mismo job volvió a responder `camera_action_required` al llamar `resume`. El RPC de seguimiento observó `waiting_camera` con la tarjeta aún montada.

El bundle local `c62308941673` incluye el backend probado y conserva el gate diario. Pasaron la suite Rust y sus diferenciales con Python, fmt, clippy y 28 pruebas Swift. La comparación DJI con imágenes coincidió con Python: con misión completó el ingreso y sin export preservó el origen. Al remontar la Sony sin formatear, el motor Rust de la app instalada confirmó las tres ausencias, los nueve hashes de cámara y `waiting_camera`, sin originales reaparecidos. Se volvieron a verificar los 12 archivos de MEDIA9 contra el recibo. La extracción física y reanudación quedaron comprobadas en el ensayo siguiente. [Plan de ensayo](../PLAN.md#ensayo-físico-de-macos).

### Lectura final Sony · 2026-11-13

En el job `17e70e69`, Python terminó el respaldo y registró el retiro de cuatro clips, cuatro XML y cuatro miniaturas. Falló en `checking_card`: el directorio exFAT aún enumeraba `C0001.MP4` y su consulta devolvía ENOENT de forma repetida. Se verificaron ambas copias contra sus recibos, las doce ausencias por ruta y el SHA-256 de los nueve archivos de cámara conservados. La comprobación corregida, ejecutada en sólo lectura sobre esa tarjeta, inventarió sólo esos nueve archivos. Python y Rust consultan de nuevo las rutas retiradas que siguen listadas. Actividad muestra la observación de tarjeta sin ofrecer reanudar un retiro ya registrado. Pasaron 72 pruebas de media Python, 16 de transferencia Rust, 23 diferenciales y 28 Swift. Falta el [ensayo físico](../PLAN.md#ensayo-físico-de-macos) con datos sintéticos y destino dedicado.

El comparativo Sony en imágenes volvió a coincidir. Después de instalar `882364a8291f`, el trabajo histórico conserva el error y registra `waiting_camera`. La app arranca el motor Rust empaquetado y SQLite devuelve `quick_check=ok`. Swift lanza Python con `-B` para preservar la firma del bundle después de ejecutar su backend.

### Aceptación local de macOS · 2026-11-13

- En una instalación aislada del bundle, **Entrada → Flujo → Resultado** revisó un plan Sony de ocho archivos con archivo ExFAT y trabajo HFS+. El primer ensayo dejó el trabajo reanudable pero mostró un error genérico al cancelar. Tras corregir SIGINT y la resolución de destinos guardados por nombre, el job `6f56c7cb` pasó de `paused/cancelled` (ocho hashes del origen intactos y sin recibos finales) a `camera_action_required` al reanudar desde Actividad. Once copias verificadas, dos recibos con el mismo digest y sin `verify_cache` débil. Al remontar la imagen sólo quedó `MEDIAPRO.XML` intacto y siguió pendiente la acción de cámara.
- En ventana, Stalker `14be9fdd` completó seis copias (manifest, pista y eventos en cada destino), y luego liberó el original. El documento Stalker con zona IANA coincidió con Python. El final publicado `376f866d` se verificó en el archivo y se retiró del disco de edición. Discos publicó índices de ambos destinos y comparó contenido. El cierre editorial y el reenlace con ACK, incluidos bloqueos y reintentos, pasaron en las suites.
- El motor firmado del bundle repitió por RPC `coverage` con y sin hash, restauró el bundle Stalker byte por byte, recuperó cuatro jobs desde recibos en otro catálogo con integridad SQLite `ok` y entregó una copia de trabajo `pending → ready` con SHA-256 comprobado. Siete comparaciones Rust/Python sobre imágenes de Sony, GoPro, H6, Canon y DJI coincidieron, incluidas las variantes DJI con y sin misión. Una prueba de integraciones tuvo un timeout de 3 s bajo carga y pasó al repetir tanto con dos hilos como con el paralelismo por defecto.
- El bundle diario y el de prueba coinciden byte por byte en Swift y Rust. SHA-256 de `Contents/Resources/backend/nuun-engine`: `fdd4147fcc96a7d51be4b7fe365aae7a069d08a7b1c7a2f691caa5aa1b0efc95`. Firma estricta de ambos ejecutables comprobada. En la app diaria arrancó el proceso Rust empaquetado y se vieron EDICION y MEDIA9 en línea. El `snapshot` directo del mismo binario devolvió 40 trabajos y 2007 assets, sin errores. SQLite dio `quick_check=ok` y mantuvo esos conteos tras el arranque. Diagnóstico mostró la build, el selector y el gate diario. Para volver, cerrar nüün y ejecutar `python3 scripts/install_media_app.py --app "$HOME/Library/Application Support/n-creator-utils/app-backups/e15c39bf.app"`.
- Hardware: sólo se observaron identidad y montaje de la SD Sony con footage real y del USB de música. Para continuar con el [ensayo físico](../PLAN.md#ensayo-físico-de-macos), conectar medios dedicados y confirmar el plan. La instalación local usa Python externo y firma ad-hoc. Distribución sigue sujeta a runtime empaquetado, Developer ID, notarización y Gatekeeper. [Gates](release.md).


### Descubrimiento y reloj de Stalker en Rust · 2026-11-13

`discover` devuelve la sesión, pista de vista previa, fecha, duración, sugerencia y etiqueta. Al elegir un archivo excluye la revisión con recibos vigentes. `plan_document` conserva el reloj del manifiesto y resuelve la zona IANA en ese instante. El diferencial comprobó el descubrimiento de una biblioteca y de un bundle, el plan en el cambio de horario de Nueva York, los hashes de ambas copias y el filtro tras archivar. Una corrida RPC en imágenes nuevas ExFAT y HFS+ produjo el mismo documento que Python, verificó ambas copias y reanudó con el original conservado. [Contrato de nombres](contracts/media-naming-v1.md).

### Índice de discos en Rust · 2026-11-13

`storage_scan` y `storage_compare` están disponibles en el sandbox de macOS. El escaneo publica en SQLite tras confirmar la identidad de la raíz y la comparación guarda cada SHA-256 terminado. El diferencial cotejó respuestas y filas de archivos, carpetas, seguridad, hashes e independencia con Python. Con carpetas temporales se verificó que cancelar o perder la raíz conserva el índice publicado, y que una comparación reanudada conserva los hashes terminados y omite archivos que cambiaron. [Mapa de almacenamiento](features/storage-map.md#índice).

### Colecciones de música en Rust · 2026-11-13

Los RPC M1 y M2 están disponibles en el sandbox de macOS. M1 vuelve a comprobar referencias de Ableton y Clipper antes de mover archivos a la Papelera; M2 conserva versiones y retoma el respaldo desde su diario. Diferencial de planes y segunda corrida incremental, y fixtures de interrupción, análisis Ableton nuevo y copia alterada. [Flujos de música](features/music-flows.md).

### Incorporación de misiones y DNG en Rust · 2026-11-13

`missions_incorporate` fusiona exports, verifica su respaldo comprimido, actualiza el espejo y retira del buzón sólo los exports verificados. `dng_plan` y `dng_apply` cubren selección de muestra y traslados a la Papelera con verificación de respaldo y diario. Los diferenciales comparan historial, cambios, export descomprimido, plan DNG y mapa de unidades contra Python. Los fixtures cubren espejo desconectado, recuperación y corrupción del respaldo. [Contrato Atlas](contracts/atlas-drone-v1.md#registro-de-misiones-en-nüün-a13).

### Retítulo, publicación y rutas Stalker en Rust · 2026-11-12

`retitle_capture` y `publish_revision` pasan a Stalker los dos descriptores de bloqueo y el token. Los fixtures probaron muerte del coordinador, reintento por el mismo ID y reconstrucción del catálogo desde recibos de una publicación. `relayout` verifica las copias, consulta dependencias de Clipper y actualiza recibos y rutas con un diario para retomar tras un renombre parcial. [Contrato de retítulo](contracts/stalker-retitle-v1.md).

### Cierre editorial completo en Rust · 2026-11-12

`closure_plan` y `closure_apply` cubren también raws, renders revisados, finales pendientes y proyectos vinculados en el sandbox de macOS. Las transcripciones se respaldan antes del retiro. Clipper aplica su propio plan de regenerables después del cierre, y un fallo permite reintentar con el mismo digest. El diferencial compara plan, aplicación y estado terminal con Python. Fixtures de corrupción, llegada de archivos y recuperación. [Contrato](contracts/project-closeout-v1.md).

### Respaldo de transcripciones en Rust · 2026-11-12

`transcripts_backup` usa el export de Clipper, liga cada transcripción a su miembro respaldado y copia con Transfer. El diferencial con Python cubre la primera copia, una versión posterior y la adopción al repetir, incluidos los recibos. Los fixtures también cubren corrupción de una copia adoptada, cambios de origen y Clipper fuera de línea. El cierre editorial utiliza este respaldo antes de liberar. [Contrato](contracts/clipper-transcripts-v1.md#3-estado).

### Cierre de finales publicados, handoff y acciones de tarjeta en Rust · 2026-11-12

`closure_plan` y `closure_apply` cubren `release_work=false`. La copia verificada y la liberación actualizan la ubicación de los finales publicados en el catálogo. El diferencial de un respaldo existente y las pruebas de copia, corrupción, cambio de decisiones y reintento tras retirar el original conservan el final sin publicar. `handoff` coincide con Python para entregas de varios miembros, bundles Stalker y errores de selección o de réplica. Se probaron expulsión y confirmación Sony con backends de fixture, incluida una lectura fallida con retiro ya registrado. La expulsión de macOS sigue pendiente de prueba física. [Métodos y variantes](../engine/README.md#métodos).

### Liberación y reenlace con Clipper en Rust · 2026-11-12

En el sandbox de macOS, Rust ejecuta `apply_release` y `relink_release`, y reutiliza la exclusión de Clipper al retirar bundles Stalker después del ingreso o limpiar finales publicados. Fixtures: un archivo de respaldo alterado bloquea todo retiro, un bloqueo que aparece durante la liberación conserva el siguiente archivo, la interrupción tras `unlink` se recupera desde el diario y un ACK con exportaciones fijadas bloquea hasta recuperar el ACK válido del mismo reenlace. Un final que llega durante la copia permanece en el origen. El cierre editorial reutiliza el mismo diario. [Contrato](contracts/media-library-v1.md#exclusión-local-de-liberación--28-octubre-2026).

### Índice observado, planes y recuperación en Rust · 2026-11-12

Alta y baja de carpetas observadas comparten el bloqueo del índice con Python. `coverage` coincide con Python con SHA-256 y en modo de tamaños. El diferencial también cubre los planes de liberación y su revisión, Stalker y finales. La recuperación de revisiones publicadas se ejercitó con recibos ordenados al revés de sus dependencias. Con fixtures, la recuperación rechaza corrupción del mismo tamaño y la biblioteca puede restaurar y entregar desde el archivo tras quitar el origen. La prueba simula un destino de trabajo desconectado y luego reintenta. Quedan la aceptación en la ventana y las pruebas físicas. [Ruta pendiente](../PLAN.md#motor-en-rust--ruta).

### Sandbox de macOS y escrituras adicionales en Rust · 2026-11-12

Las cuatro imágenes montadas de la sesión anterior pertenecen al sandbox original. Las pruebas se hicieron en un sandbox temporal independiente. El backend comprueba el catálogo efectivo y rechaza volúmenes ajenos al sandbox. Swift enruta escrituras declaradas a Rust en ese entorno y sólo reintenta con Python ante `not_ported`. Se portaron `closure_save` y el RPC independiente de `backup_metadata`. La build de desarrollo `bdc7221c6f67` está en `dist/nuun-rust-sandbox-2026-11-12.app`; queda pendiente el recorrido manual de la ventana. [Comandos del sandbox](../engine/README.md#sandbox-de-macos).

### E1a · escrituras del catálogo en Rust · 2026-11-12

Sólo `engine/` y el dorado; sin cambios en la app ni reinstalación. Rust hace 14 escrituras que sólo tocan el catálogo (más `defaults` e `integration_settings` con parámetros): perfiles, dispositivos confirmados, estado y nombre de proyectos (con alias, etiquetas y valores por defecto que lo siguen), trabajo pendiente u omitido, publicaciones, etiquetas de captura, carpeta de publicados, vínculos de cierre, trabajos ocultos, planes cancelados y sesiones descartadas. Toma el lock de escritura donde Python lo toma y en ningún otro lado. Dorado nuevo: el mismo catálogo base y 43 escrituras, entre válidas y rechazadas, dejan **el mismo catálogo que Python**, tabla por tabla (metadatos, proyectos, alias, assets, trabajo, publicaciones y eventos), con las mismas respuestas. 129 pruebas de Rust. Esa entrega preparó las escrituras que el sandbox de Mac usa ahora.

### E0b · `sources` y la guarda de Clipper en Rust · 2026-11-12

`sources` (las fuentes de un proyecto para un editor) y `capture_discard_check`, con `clipper.inspect` portado completo: el reporte de Clipper, proyectos antiguos que no reporta, registro de vinculados y cola de exportación. Dorados nuevos: 4 consultas de `sources` sobre discos de prueba y un Clipper de prueba con su `HOME`, que da 4 `inspect` y 5 comprobaciones de descarte; todos iguales a Python. Repetición real: `sources` igual en 12 consultas (279 contra 595 ms). En la app, `capture_discard_check` sigue en Python hasta compararlo con una sesión real (ahora no hay pendientes). `coverage` pasa a E3. 128 pruebas de Rust, 28 de Swift. Instalado como `cdc9a9651266`.

### E0a · lecturas de un solo uso en Rust y finales en Inicio · 2026-11-12

Instalado como `c698d704f4de` el 12 nov. (1) **Arreglo:** los «finales por respaldar» no aparecían al abrir Inicio hasta cambiar de pestaña. Inicio los pedía antes de que el primer `snapshot` supiera qué discos estaban en línea; ahora se piden también al terminar el `snapshot`, con el mismo límite de 15 minutos. (2) **Motor:** `finals_status`, `captures` (con `changed_paths`), `publications`, `retitle_operations`, `collection_release_plans` y `resolve` responden por `rpc`, como en Python, con casos nuevos en tres dorados. Repetición real, un proceso por petición: todo **igual**; `finals_status` 330 contra 725 ms, las pequeñas 5 contra 85–95 ms. (3) **App:** `BackendClient` pide a Rust primero las lecturas de un solo uso que el motor declara (`engine_info.once`), salvo `discover`, y Python responde si no; las escrituras nunca van a Rust. Pruebas: 126 de Rust, 28 de Swift. Queda E0b: `sources`, `coverage`, `capture_discard_check`.

### La app de Mac lee con el motor en Rust (paso 7) · 2026-11-12

Instalada el 12 nov como **`b58b8360a1b8`** (anterior en `app-backups/d3451bed.app`). Con el binario instalado, el `snapshot` completo tarda 0,58 s frente a 1,75 s de Python; la primera ejecución tardó 4,9 s porque macOS revisa una vez un binario nuevo. `PersistentReader` (Swift) lleva dos procesos persistentes: `nuun-engine rpc-serve`, al que se le pide primero cada lectura que declara en `engine_info`, y el `rpc-serve` de Python, que responde lo que Rust no tiene, un `not_ported` o una caída del proceso. Un error del propio método (proyecto desconocido, disco desconectado) es la respuesta, como lo sería en Python. Las escrituras siguen en Python (`BackendClient`). Ajustes → Diagnóstico: selector «Lecturas» (Rust con respaldo / Python) y quién respondió cada método. `build_media_app.py` compila el motor y lo firma dentro de `Resources/backend/nuun-engine`; el `Nu9veBuildID` incluye sus fuentes; `--no-engine` lo deja fuera. Evidencia: 27 pruebas de Swift, localización en inglés completa (863 claves), build de prueba con el motor firmado y verificado (backend `macos`, 32 lecturas, sin escrituras). Falta la aceptación en la app: abrirla, revisar Ajustes → Diagnóstico y una semana de uso.

### Motor en Rust: todas las lecturas · 2026-11-12

Sólo `engine/` y los scripts del dorado y la sombra; sin cambios en `media/` ni en la app, y sin escribir en ningún disco ni índice. Se portaron las últimas lecturas: `closure_candidates`, `storage_overview`/`storage_progress`/`storage_tree` (el índice de discos se abre sólo para leer; si falta, cuenta como vacío) y `missions_review` (el merge del registro de misiones, sobre una copia en memoria). Hay dos dorados nuevos, misiones y almacenamiento, más cuatro búsquedas de candidatos en el de proyectos: 10 diferenciales en total, todos verdes. Sombra sobre el catálogo real, el índice de 205 mil archivos y `~/Downloads`: **todas las lecturas iguales a Python**. La única diferencia es el espacio libre del disco interno, que cambia entre una consulta y otra. Tiempos: `closure_candidates` 292 contra 817 ms, `storage_overview` 395 contra 638, `missions_review` 373 contra 539. 126 pruebas de Rust. Lo que falta en Rust son las escrituras ([PLAN](../PLAN.md#motor-en-rust--ruta)).

### Motor en Rust: backend de volúmenes de macOS (paso 6) · 2026-11-12

Sólo `engine/` y los scripts del dorado y la sombra; sin cambios en `media/` ni en la app, y sin escribir en ningún disco. `MacBackend` describe volúmenes con las mismas fuentes que Python (`statfs`, `diskutil -plist`, `statvfs`), así que lo registrado por Python queda en línea sin registrarlo otra vez. Los discos se describen en paralelo. Las escrituras quedan cerradas en la Mac hasta la corrida física. Con él se portaron `capture_overview`, `closure_deliveries` (todos los estados de una entrega final) y `suggest`, más un dorado de proyectos. Sombra sobre el catálogo real con MEDIA9 y EDICION montados (sólo lectura): **todo igual a Python**, incluido el `snapshot` completo, en 601 ms contra 1645 ms; `source_cards` en 343 contra 1170 ms. 122 pruebas de Rust, 8 diferenciales. Quedan `closure_candidates`, `missions_review` y `storage_*` ([PLAN](../PLAN.md#motor-en-rust--ruta)).

### Motor en Rust: fase L, lecturas sin discos · 2026-11-12

Sólo `engine/` y los scripts del dorado y la sombra; sin cambios en `media/` ni en la app. Diez lecturas más en Rust (lista en la fila del motor), con dos dorados nuevos: uno de catálogo (29 peticiones sobre el SQL de un catálogo de Python) y uno de bibliotecas de Stalker (9 sesiones en disco). Siete diferenciales en total, todos verdes; `--check` sigue determinista (los inodes de las capturas se numeran por ruta). Sobre el catálogo real, todo lo portado responde **igual** que Python, incluido `integration_capabilities` contra Clipper y Stalker instalados. Tiempos: `capabilities` tarda ~0,5 s en los dos (es Clipper); el `snapshot` de Rust la corre en paralelo (537 ms) contra 1644 ms de Python, que además revisa los discos. Quedan seis lecturas que dependen del paso 6 ([PLAN](../PLAN.md#motor-en-rust--ruta)). 117 pruebas de Rust.

### Motor en Rust en la Mac: compila, dorado y sombra · 2026-11-11

Sin cambios de comportamiento en la app ni reinstalación. (1) **Paso 0**: `engine/` compila y pasa en macOS sin descargas; clippy encontró un `///` sobre código sólo de Unix que Windows nunca compiló. (2) **Paso 2, dorado**: `scripts/engine_golden.py` ya corre en la Mac y es determinista (raíz fija, inode/ctime/ids por contador, WAV de 8 kHz: de 1,8 MB a 165 KB), con `--check` para CI; las 5 diferenciales pasan. Las dos diferencias que aparecieron eran del arnés de la prueba del planner (raíz de Python sin normalizar y un `apply` sobre stamps de Python), no del motor. (3) **Sombra**: [media/shadow.py](../media/shadow.py) y [scripts/engine_shadow.py](../scripts/engine_shadow.py); apagada salvo que se active. La primera repetición sobre el catálogo real encontró dos errores de Rust, ya corregidos: `projects` ordenaba los empates por id (Python conserva el orden de la biblioteca) y `living_collections` (M2) no estaba en su lista de lecturas, así que la rechazaba como escritura; `tests/test_engine_boundary.py` evita que las listas se separen otra vez. (4) **CI** de GitHub escrita. (5) Decisión del usuario: lo nuevo entra en Rust. Falta: correr la CI, activar la sombra en uso real y portar las lecturas que faltan ([PLAN](../PLAN.md#motor-en-rust--ruta)).

### Bases del motor en Windows antes de pasar a la Mac · 2026-11-11

Sólo `engine/`, scripts y configuración del repo; sin cambios en `media/`, en la app de Mac ni en la ventana de Windows. (1) **Verificación sin caché** ([A1](../vendor/nu9ve-core/docs/audits/fable-audit-2026-11-10.md#a1--la-relectura-de-verificación-no-evita-la-caché-fuera-de-macos)): `fs::verify_read` reabre el archivo ya validado con `FILE_FLAG_NO_BUFFERING` en Windows y `posix_fadvise` en Linux (macOS sigue con `F_NOCACHE`); la usan la relectura de parciales, la adopción de destinos y la relectura del original antes de limpiar. Si el sistema de archivos lo rechaza, el recibo y el resultado dicen `verify_cache`; si no, quedan como los de Python. Fila nueva en el gate de pérdida de datos de [release](release.md). (2) **Higiene (S3, parte):** `.gitattributes` con LF (el índice ya era LF; no cambió ningún archivo), `engine/rust-toolchain.toml` (1.99.1) y `native/windows/global.json` (SDK 10.0.403). (3) **Backend por capacidad** ([B6](../vendor/nu9ve-core/docs/audits/fable-audit-2026-11-10.md#b6--el-motor-decide-por-cfgwindows)): el binario ya no tiene `cfg(windows)`; S16 sólo agrega el backend de macOS en `platform_backend()`. (4) `scripts/check_suite_boundary.sh`: ninguna pieza `nu9ve-*`/`Nu9ve.*` nombra algo de nüün. Evidencia en la fila del motor. Lo que no se pudo aquí: compilar en macOS o Linux, el dorado y una tarjeta real.

### Auditoría Fable de la suite · 2026-11-10

Sólo documentación, escrita en el PC de Windows leyendo nüün, nove, curado e `ios-fractos`; **nada se compiló ni se ejecutó**. [Auditoría](../vendor/nu9ve-core/docs/audits/fable-audit-2026-11-10.md) con hallazgos priorizados, [integración por sistema operativo](../vendor/nu9ve-core/docs/platforms/os-integration.md), [Linux](../vendor/nu9ve-core/docs/platforms/linux.md), [sonido de tema v1](../vendor/nu9ve-core/contracts/nu9ve-sound-v1.md) (propuesta) y [briefs por sesión](../vendor/nu9ve-core/docs/audits/session-briefs-2026-11-10.md); en `ios-fractos`, `docs/SUITE-DESKTOP.md`. Segunda pasada el mismo día, con la app de Mac de Fractos ya en su remoto (`f1a7bcb`): el contrato de sonido toma el motor v2 de Fractos como fuente, y las tablas de adopción de [Mac](../vendor/nu9ve-core/contracts/macos-app-v1.md#adopción), [tema](../vendor/nu9ve-core/contracts/nu9ve-theme-v1.md#adopción) e [interacción](../vendor/nu9ve-core/contracts/nu9ve-interaction-v1.md#adopción) tienen su columna de Fractos leída en el código. Tercera pasada: decisión del usuario de **sonido encendido por defecto con «Silenciar» a un paso** (tema §12 y sonido §7 actualizados), nuevo [estándar de avisos del sistema](../vendor/nu9ve-core/contracts/nu9ve-notifications-v1.md) (propuesta, con nove en Windows como base) y [propuesta del repo común](../vendor/nu9ve-core/docs/suite-sharing.md). En nüün para Mac hoy sólo existe el aviso de tarjeta detectada; el de trabajo terminado no está implementado. La relectura de verificación sin caché en Windows quedó corregida el 11 nov (entrada de arriba). Revisión del 11 nov, sólo documentación: se confirmó ese hallazgo en el código y que la ventana de Windows aún no llama a `plan`/`apply`; se actualizaron las filas del motor y de Windows y los límites 4 a 6 (el 5 ya no nombra el `.gitlab-ci.yml`), y el PLAN tiene ahora la [ruta del motor](../PLAN.md#motor-en-rust--ruta) en dos fases: usable en Windows y sustituir a Python en Mac.

### M2 · carpeta viva · 2026-11-09

Sólo checkout, sin reinstalar la app (sin cambios de interfaz). `media/living.py` + CLI/RPC y la tabla aditiva `living_files`. Hace un respaldo incremental y verificado de una carpeta que cambia, hacia una carpeta de un archivo registrado. No renombra, no limpia el origen ni propaga borrados, y la versión anterior de un archivo cambiado se aparta dentro del archivo (`_nuun/versions`). Un archivo editado mientras se revisa el plan se salta en lugar de abortar. Incluye un informe de muestras externas de los sets de Ableton. Detalle, colecciones recomendadas y runbook en [music-flows](features/music-flows.md#m2--carpeta-viva-implementado-2026-11-09).

### M1 · liberar una colección contra archivos registrados · 2026-11-09

Sólo checkout, sin reinstalar la app (no hay cambios de interfaz; la build instalada `a350af7f16af` todavía no trae este módulo). `media/collection_release.py` + CLI/RPC. Libera de una carpeta viva (SONIDO) sólo lo que tiene una copia idéntica por SHA-256 en MEDIA7/501/502, aunque tenga otro nombre. Conserva lo que tiene `.asd`, lo que usa un `.als` o Clipper, y lo que se pida. Mueve a la Papelera del volumen con journal. Dos defectos que encontraron las pruebas y ya quedaron corregidos: el nombre de una referencia de Ableton viene con separadores `:` (rutas HFS) y la primera consulta a Clipper podía saltarse por el reloj monotónico. Las pruebas aíslan `Path.home` y `CLIPPER_PROJECTS_ROOT`; un intento previo sin aislar llegó a leer (sólo lectura) el registro real de Clipper. Detalle y runbook en [music-flows](features/music-flows.md#m1--liberar-una-colección-implementado-2026-11-09).

### Motor en Rust: adoptado y primer método · 2026-11-09

El usuario adoptó la ruta. Nuevo `engine/` (`nu9ve-json`, `nu9ve-volumes`, `nuun-recipe`, `nuun-engine`), sin cambios en `media/` ni en la app de Mac. `recipe` responde desde Rust con la forma de Python (digest de la plantilla `36f803f0…`, por confirmar contra Python con `scripts/engine_golden.py`). La ventana de Windows lo usa método por método (`MethodByMethodEngine`). Dependencias: las mismas versiones que nove y curado, ya en caché; ninguna descarga.

### Contrato MCP de la suite y análisis del motor en Rust · 2026-11-09

Sólo documentación, sin cambios de código. [nu9ve-mcp-v1](../vendor/nu9ve-core/contracts/nu9ve-mcp-v1.md) toma el MCP de nove (niveles, permiso por sesión, cambios de agente deshacibles) como estándar, con estado por app; `nove_status` respondió desde un cliente Claude real en esta máquina. [Motor en Rust](platforms/rust-engine.md): SHA-256 medido a 1,67 GB/s en Rust `sha2` y en .NET; la ganancia de migrar es distribución y motor común, no la verificación. Recomendación por fases con prueba diferencial contra Python; decisión pendiente.

### Base de Windows y maqueta de nüün · 2026-11-09

Sin cambios en `media/` ni en la app de Mac. Nuevo `native/windows`: `Nu9ve.Core` (port literal de `ThemeRoles`/skins de Swift, superficies por skin, reglas de atajos, ventana, instalador), `Nu9ve.WinUI` (tema → pinceles, controles con skin, franja de época, cromo DWM), `Nuun.Core` (transporte de `media rpc-serve`, maqueta con la forma real de `snapshot`/`recipe`), la app y `NuunSetup.exe`. Vectores de tema compartidos en [`contracts/examples/theme-vectors-v1.json`](../vendor/nu9ve-core/data/theme-vectors-v1.json) (C# verificado; Swift pendiente). La prueba de contraste por superficie encontró cuatro estados presionados ilegibles, corregidos antes de verlos. Scripts en `scripts/windows`. Aprendizajes en [Windows en la suite](../vendor/nu9ve-core/docs/platforms/windows.md); propuestas para [nove](../../../ai/nove/docs/suite-alignment-windows.md) y [curado](../../../ai/curado/docs/SUITE-WINDOWS.md).

Las entregas del 8 nov y anteriores están en la [bitácora](releases/status-log-2026-10-25_2026-11-08.md).
