# Inventario de música en SSD1 · 2026-11-09

Lectura de sólo nombres, tamaños y fechas (`find` + `stat`), sin abrir archivos ni escribir en ningún disco. Las coincidencias son por **nombre + tamaño**: sirven para orientar, no para liberar nada. Liberar exige hash actual ([política](replication-policy.md)). Diseño de producto en [carpetas vivas](product-flows.md#carpetas-vivas-música-y-samples); propuesta y respuestas del usuario en [music-flows](music-flows.md).

## Lo urgente

- **SSD1 está al 99 %:** 918 de 932 GB usados y 13 GB libres.
- **Casi nada tiene respaldo.** De 853 GB de audio en SSD1, sólo ~15 GB aparecen en MEDIA502 con el mismo nombre y tamaño. El resto es copia única.
- **MEDIA502 tiene 2.1 TB libres**, así que cabe todo.
- **~60 GB están duplicados dentro del mismo SSD1** (5,611 grupos con el mismo nombre y tamaño), sobre todo entre carpetas de `mix/` y entre `mix/` y `selec/`.

## Qué hay, por colección

| Colección | Carpetas | Tamaño | Contenido | En MEDIA502 |
| --- | --- | --- | --- | --- |
| **Biblioteca DJ / curado** | `mix/` (90 subcarpetas), `selec/`, `hijack/`, `splits/`, `zelda/` | ~392 GB | 25k MP3, 4k FLAC, WAV/AIFF/M4A/DSF/WMA. Casi todo con fecha de mayo 2023 (copia masiva); `mix/ton` es de 2025 | ~4 GB |
| **Grabaciones largas** | `recordingslong/2019…2026`, `podcasts` | 256 GB | 581 WAV por año/mes | 2026 sí (10 GB); 2019–2025 no |
| **Grabaciones cortas** | `recordings/2021…2026`, `chuly/`, `leplebe/`, `demos/`, `Contents/` | ~14 GB | WAV | ~1 GB |
| **Producción Ableton** | `abletonlive/projects` (29 proyectos, 1,815 `.als` con versiones), `collabs`, `recordings`, `stems`, `canciones`, `visuals`, `max` | ~85 GB | Sets y su audio | No |
| **Librerías de sonidos** | `abletonlive/packs` (87 GB), `samples` (55 GB), `kontakt`, `Spitfire`, `loops`, `samplepacks/`, `sounds/` | ~155 GB | 32k AIF, 35k WAV, ZIP/RAR. Muchas se pueden volver a descargar | No (`sounds/` sí) |
| **Fuera de música** | `zpluginz/` (instaladores RAR/ISO, 11 GB), `cue/` (MP4 + VTT, 7.5 GB) | ~18 GB | Instaladores y video | No |
| **Datos de DJ** | `.PIONEER/` (exportación de Rekordbox para USB), `_Serato_`, `_Serato_Backup` | ~45 MB | Bases y análisis | No |

En MEDIA502 ya hay música en `000_acomodar/audio` (29 GB), `002_finales/9sets` (153 GB), `002_finales/a9sets` (22 GB) y `002_finales/collabs` (113 GB), además de `001_raw/recordings*`.

## Duplicados más grandes dentro de SSD1

| Espacio extra | Dónde |
| --- | --- |
| 5.5 GB | `mix/defectedhouseands` + `mix/sleccct20222` |
| 5.2 GB | `recordingslong/2019` + `recordingslong/podcasts` |
| 5.0 GB | `mix/05032022` + `mix/aaa180222` |
| 3.5 GB | `mix/erikiado` + `selec/CLABOSVIBES` |
| 2.8 GB | `abletonlive/canciones` + `abletonlive/collabs` |
| ~10 GB | Varias carpetas de `mix/` + `selec/CLABOSVIBES` |

## Riesgo antes de reorganizar: Rekordbox y Serato

Rekordbox (y Serato) guardan **rutas absolutas**. Si se mueve o renombra un archivo de `mix/` o `selec/` desde Finder o desde nüün, la pista queda «perdida» en su colección, con sus cues, beatgrids y playlists. Por eso hay que respaldar antes de reorganizar, y reorganizar sólo con una herramienta que también actualice las rutas: exportar el XML de Rekordbox, mover según un plan y generar un XML con las rutas nuevas para importarlo. `master.db` está cifrado; la interfaz segura es el XML. `.PIONEER/` en la raíz indica que SSD1 también se usa como USB de exportación.

## Propuesta en orden

1. **Respaldar sin tocar SSD1** (carpeta viva → MEDIA502, sin renombrar ni limpiar, sólo lo nuevo en cada corrida). Por riesgo y valor:
   1. `recordingslong` y `recordings` (270 GB, irrepetibles). Ya existe `001_raw/recordingslong` en MEDIA.
   2. Producción Ableton (proyectos, collabs, stems, canciones, grabaciones; ~85 GB).
   3. Biblioteca DJ (`mix`, `selec`, …; ~392 GB) → `003_curado/musica/` u otra raíz que decidas.
   4. Librerías de sonidos (~155 GB), al final: parte se puede volver a descargar.
2. **Liberar espacio verificado:** después de respaldar, quitar de SSD1 los años viejos de `recordingslong` (2019–2024, ~230 GB) con rehash. Es lo único que libera mucho sin afectar Rekordbox.
3. **Duplicados:** confirmar por hash los ~60 GB, elegir cuál se queda (la ruta que usa Rekordbox) y mandar el resto a la Papelera con un plan revisado.
4. **Reordenar la biblioteca DJ** con la herramienta compatible con Rekordbox; las 90 carpetas de `mix/` se agrupan por género, set o época según decidas.
5. **Librería de producción:** índice de samples, loops, one-shots y MIDI por tipo y duración, separado de la biblioteca de curado.

## Herramientas que harían falta en nüün

| Herramienta | Qué hace | Reutiliza |
| --- | --- | --- |
| Carpeta viva | Copia incremental verificada a MEDIA, sin renombrar ni limpiar ni propagar borrados | planner/transfer, `source_kind` colección (fase 3 de [product-flows](product-flows.md#fases)) |
| Cobertura de colecciones | Este informe como vista: tamaño, tipos, qué ya tiene copia, duplicados (nombre+tamaño → hash) | `storage_index`, `coverage` |
| Deduplicador con plan | Agrupa por hash, propone cuál conservar y manda a la Papelera las demás tras revisión | digest/revisión/journal existentes |
| Reubicación compatible con Rekordbox | Lee el XML exportado, mueve según un plan y escribe un XML con las rutas nuevas | Nuevo; nunca escribe `master.db` |
| Índice de samples | Clasifica packs, samples, loops y MIDI, consultable desde la app | Nuevo, sólo lectura |

## Preguntas para el usuario

- ¿La biblioteca que usas en vivo es la de Rekordbox, la de Serato o ambas? ¿Las pistas apuntan a `SSD1/mix` y `SSD1/selec`?
- ¿`recordingslong` son grabaciones de sesiones o sets, y se pueden liberar del SSD1 una vez respaldadas?
- ¿Dónde quieres la biblioteca de curado en MEDIA (`003_curado/musica`)? ¿Y la de producción?
- ¿Los packs de Ableton/Kontakt/Spitfire se pueden volver a descargar, o los respaldamos igual?
