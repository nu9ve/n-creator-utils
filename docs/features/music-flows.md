# Música: liberar, analizar, respaldar y organizar

Propuesta del 2026-11-09, a partir del [inventario de SSD1](music-ssd1-inventory-2026-11-09.md) y de lo que dijo el usuario. Nada de esto está implementado. Prioridad en [PLAN](../../PLAN.md).

## Quién es dueño de qué

| Dueño | Qué |
| --- | --- |
| **nüün** | Copias, ubicación, respaldo verificado, duplicados por hash, mover y liberar con plan revisado |
| **curado** | Estado de la biblioteca: tags, colecciones, playlists, cues, BPM/tono, relink por huella, lectura de USB rekordbox y escritura de `rekordbox.xml` |
| **Clipper** | Motor de análisis de audio (envolvente RMS `rms-envelope-v1`) y transcripción local (`mlx_whisper`, `qwen3_asr_mlx`) |
| **Ableton** | Proyectos `.als` y sus referencias. nüün sólo las lee |

Ninguna app escribe en la base de otra. nüün nunca escribe en `master.db` de rekordbox ni en la base de curado.

## 1 · Liberar `recordingslong` y `recordings` de SSD1

Lo que dijo el usuario: estas carpetas salieron de un `save all` y su respaldo está en MEDIA500 o MEDIA501. Se conserva lo que use un proyecto (`.asd` o algo así), y quizá se borra todo 2019–2021.

Medido el 9 nov (sólo lectura). Todos los WAV tienen el nombre fechado del importador anterior (`23MAY10-014910_H6LR_67-47_JP-SHINKANSEN[].WAV`). Se leyeron los 2,879 `.als` de SSD1 y `~/Music`: **141 grabaciones** están referenciadas por un set (por ejemplo, `erikiado`, `soltura`, `errorton`).

| | Total | Proteger (`.asd` o usado por un set) | Candidatos |
| --- | --- | --- | --- |
| `recordingslong` 2019–2021 | 72 GB | 6.3 GB (25) | 65.7 GB (179) |
| `recordingslong` 2022–2025 + podcasts | 173 GB | 0.8 GB (2) | 172.7 GB (358) |
| `recordingslong` 2026 | 10 GB | 3.4 GB | 6.6 GB (ya está en MEDIA502) |
| `recordings` | 7 GB | ~0 | 6.9 GB |
| **Total** | **262 GB** | **10.5 GB** | **~252 GB** |

**Cómo:** se reutiliza la liberación que ya existe ([release.py](../../media/release.py)). Busca cada archivo **por contenido** (SHA-256) en un archivo registrado, aunque allá tenga otro nombre, y no borra sin coincidencia. Falta:

1. Registrar MEDIA500 y MEDIA501 como archivos cuando estén conectados. Sólo se leen.
2. Permitir liberar una carpeta de un disco de colección (SSD1), no sólo de un disco de trabajo.
3. Una lista de conservación: `.asd` junto al WAV, referencias desde `.als` y, si se quiere, años o carpetas elegidos. El plan la congela en su digest.
4. Lo que no aparezca en MEDIA500/501 no se borra: se respalda primero en MEDIA502.

Resultado: un plan revisable por año, con «se libera (hash coincide en MEDIA50x)», «se conserva (lo usa un set)» y «sin respaldo». Lo liberado va a la Papelera del SSD1, con journal.

## 2 · Análisis de audio: relleno o importante

La idea es ordenar la revisión, no decidir sola: el análisis nunca borra.

| Nivel | Cómo | Costo | Responde |
| --- | --- | --- | --- |
| 0 · Metadatos | Duración, canales, nombre fechado, lugar (`JP-SHINKANSEN`) | Nada | Agrupar por viaje o sesión |
| 1 · Señal | `ffmpeg` `ebur128`/`astats`/`silencedetect`, o la envolvente RMS de Clipper | Segundos por hora de audio | Silencio o ruido constante (probable grabación por error), dinámica, picos |
| 2 · Voz o música | Proporción de voz (VAD) y espectro | Bajo | Voz, música, ambiente |
| 3 · Transcripción | `qwen3_asr_mlx` o `mlx_whisper` (Clipper), sólo en lo que tiene voz | Alto | Qué se dice; búsqueda; resumen |

**Dueño:** Clipper ya tiene el motor. Para no duplicarlo, nüün le pide un análisis por CLI (como `cleanup-plan`) con un contrato nuevo, `audio-triage-v1`: entrada, una lista de archivos con hash; salida, clases y medidas por archivo, con `engine` y versión. nüün lo guarda en su catálogo y lo muestra como filtro (Silencio, Ruido, Voz, Música) en la revisión de un plan de liberación o de respaldo. Las transcripciones durables siguen el [contrato de transcripciones](../contracts/clipper-transcripts-v1.md).

## 3 · Respaldar la producción de Ableton

| Dónde | Qué | Tamaño |
| --- | --- | --- |
| `SSD1/abletonlive/projects`, `collabs`, `stems`, `canciones`, `recordings` | 29 carpetas de proyecto, 1,815 `.als` con versiones | ~85 GB |
| `~/Music/01_produccion` (`splits`, `drums`, `shots`, `loops`, `samples`, `midis`, `AXOMODAR`) | Material de producción ya ordenado por tipo | 42 GB |
| `~/Music/03_final` (`projects`, `tracks`) | Finales | 9 GB |
| `SSD1/abletonlive/packs`, `samples`, `kontakt`, `Spitfire` | Librerías | ~155 GB |

**Cómo:** con una carpeta viva, que copia a MEDIA502 sólo lo nuevo o cambiado, verificado, sin renombrar ni limpiar el origen y sin propagar borrados. La carpeta de proyecto de Ableton se trata como un bundle y se copia completa. Antes de copiar, un informe de **referencias externas**: qué muestras usa cada set fuera de su carpeta (packs, `recordingslong`, `~/Music`) y si esas muestras ya tienen respaldo. Así un set respaldado se puede abrir. «Collect All and Save» sigue siendo una acción de Ableton que hace el usuario.

## 4 · Organizar la biblioteca de música

Hoy `mix/` tiene 90 carpetas, una por tanda de descargas, con unos 60 GB duplicados. Además hay música en `selec/` y `~/Music`, y descargas de YouTube con poca metadata.

**Principio:** las carpetas responden a *de dónde vino y dónde está*, que no cambia. Los tags y las colecciones responden a *qué es y para qué la uso* (género, energía, set), que cambia y admite varias respuestas a la vez. Si se ordena por género en carpetas, cada pista acaba en un solo lugar y hay que mover archivos cada vez que cambia de criterio.

Estructura propuesta (la misma en MEDIA502 y en el disco de trabajo):

```text
musica/
  entrada/AAAA-MM_<origen>/      # lo recién llegado tal como vino: youtube, bandcamp, soundcloud, compras
  biblioteca/<Artista>/<Título>.<ext>        # ya con tags mínimos (artista, título); A–Z en Finder
  biblioteca/_sin-artista/<Título>.<ext>     # lo que no se pudo etiquetar todavía
produccion/
  proyectos/<nombre> Project/    # bundles de Ableton
  grabaciones/AAAA/MM/           # nombres fechados de nüün
  stems/ · splits/
sonidos/
  packs/<marca> - <pack>/        # tal como se instalan
  drums/ · shots/ · loops/ · samples/ · midis/   # la estructura que ya usas en ~/Music/01_produccion
```

- **Las 90 tandas de `mix/` no se pierden:** cada una se vuelve una colección en curado (`lote: aaa180222`), así que la procedencia queda aunque los archivos se muevan.
- **Género, energía, BPM, tono y sets** son tags y colecciones de curado (ya tiene tablas `tags` y `collections`, y análisis BPM/tono). Las playlists de rekordbox salen de curado como `rekordbox.xml`.
- **Descargas de YouTube:** se normaliza el tag al descargar (`yt-dlp --embed-metadata` y «Artista - Título» del título del video). Lo que no se pueda separar va a `_sin-artista` hasta revisarlo.
- **Duplicados:** primero por hash de bytes y después por huella de audio (curado ya la calcula), para detectar la misma canción en MP3 y FLAC. Se conserva la de mejor calidad o la que usa rekordbox.

**Mover sin romper rekordbox ni curado:** nüün mueve sólo con un plan revisado y deja un mapa `ruta_anterior → ruta_nueva` con hash. curado reubica con ese mapa (ya tiene relink por huella) y exporta `rekordbox.xml` con las rutas nuevas, que se importa en rekordbox. Un contrato `music-library-v1` define el mapa y el ACK de curado, igual que el relink de Clipper. curado ya lo tiene en su roadmap («nüün: leer de dónde viene un archivo»).

## Orden propuesto

1. **Liberar `recordingslong`/`recordings` contra MEDIA500/501** (sección 1). Es el mayor alivio (~250 GB) y reutiliza la liberación existente. Necesita conectar esos discos.
2. **Carpeta viva** para producción y `~/Music` hacia MEDIA502, con el informe de referencias externas.
3. **Triage de audio** con Clipper (`audio-triage-v1`), primero los niveles 0–1.
4. **Biblioteca:** respaldar `mix/` y `selec/` tal como están, después deduplicar y luego reorganizar con curado y el contrato `music-library-v1`.
