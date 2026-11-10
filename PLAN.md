# Plan de trabajo

Actualizado: 2026-11-08, America/Hermosillo. Éste es el backlog priorizado. [STATUS](docs/STATUS.md) dice qué funciona hoy y con qué evidencia; [CONTEXT](docs/CONTEXT.md) guarda las decisiones. La historia por entrega está en [releases/](docs/releases/status-log-2026-10-25_2026-11-08.md) y no es otra lista de pendientes.

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
  1. Decidir dónde viven `Nu9ve.Core`/`Nu9ve.WinUI` y cómo se versionan antes de que nove los copie.
  2. Que `ThemeRolesTests.swift` lea los vectores compartidos.
  3. Adaptador de volúmenes de Windows en `media/` (serial de volumen, letras, expulsión), con fixtures.
  4. Python empaquetado y contrato contra el motor real.
  5. Firma e instalación del MSIX y del Setup en una máquina limpia.
- **Motor en Rust: adoptado por el usuario el 2026-11-09.** [Análisis y ruta](docs/platforms/rust-engine.md). Razones: distribución sin Python, un motor para Windows y Mac, piezas compartidas con nove y curado. **No** la velocidad de verificación (medido: SHA-256 a 1,67 GB/s en Rust y .NET; el disco manda). Python es el oráculo hasta la paridad de cada método; la app de Mac cambia de motor método por método. En curso:
  - **Fase 0:** `scripts/engine_golden.py` genera las respuestas doradas. Falta correrlo en la Mac y medir cada fase en una corrida física pedida.
  - **Fase 1:** [`engine/`](engine/README.md) tiene el JSON canónico compatible con Python, la receta (con lo guardado en el catálogo) y, en Windows, `volumes` y `source_cards` con una identidad de volumen propia (GUID, serial, discos físicos, bus). Sigue `register_volume` (primera escritura, sólo configuración), después `snapshot` y búsqueda de biblioteca.
- **MCP** ([contrato v1](docs/contracts/nu9ve-mcp-v1.md)): que `media mcp` pida al lector persistente en vez de abrir el catálogo, `serverInfo` `nuun`, campos `_untrusted`, `show_in_app` para un proyecto o un plan revisado y `docs/mcp.md`. Nunca copiar, limpiar ni liberar por MCP.

## Prioridad baja · nombre interno

Renombrar `MediaMenu` → `Nuun` (paquete, targets, ejecutable, prefijo `Nu9veMedia-local-*`, scripts y docs) en un cambio propio sin comportamiento nuevo. Se conservan `CFBundleIdentifier` `app.nu9ve.media`, `~/Applications/nuun.app` y el catálogo.

## Retomar en otra sesión

Leer CONTEXT → STATUS (tabla por área) → fila de este PLAN → contrato o módulo dueño. Registrar sólo resultados observados con job/build; no heredar aprobación física de otra build o de fixtures. Respetar `git status`, sin commits/restage ni mensajes a otras sesiones como efecto lateral. [Formato de handoff](docs/development.md#cerrar-y-retomar-una-sesión).
