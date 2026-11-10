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

## Decisiones del usuario · 2026-11-09

- `recordingslong` y `recordings` salieron de un `save all`; los años pasados ya tienen respaldo en **MEDIA500 o MEDIA501**. Se pueden quitar de SSD1 la mayoría, sobre todo 2019–2021, **salvo lo que use un proyecto** (`.asd` junto al WAV o referencia desde un `.als`).
- Nada se borra sin encontrar la copia **por hash** en un archivo registrado; si no aparece, primero se respalda en MEDIA502.
- Biblioteca: rekordbox sí, pero sobre todo **curado** a futuro. Serato casi no se usa; sus carpetas no son la biblioteca.
- Hay producción y música también en `~/Music` (`01_produccion`, `03_final`, `00_assets`): entra en el respaldo de producción.
- Interés en un triage de audio (silencio, ruido, voz, música) con `ffmpeg` y Qwen para decidir qué vale la pena; lo hace Clipper, nüün lo muestra.
- Organizar `mix/` para que el respaldo quede navegable en Finder; las tandas de descarga se conservan como colecciones, no como estructura final.
- Trabajo por fases. Al cerrar cada fase: decir cómo terminó y cómo continuar, con enlaces a los docs.

## Fases

Cada fase cierra con pruebas en fixtures, docs al día (esta página, [STATUS](../STATUS.md), [PLAN](../../PLAN.md)) y un resumen. Las corridas sobre discos reales son operaciones que el usuario pide, con plan revisado.

| Fase | Entrega | Criterio de cierre | Estado |
| --- | --- | --- | --- |
| M1 · Liberar grabaciones | Liberación de una carpeta de un disco de colección (SSD1) contra archivos registrados (MEDIA500/501/502) por hash, con lista de conservación (`.asd`, referencias `.als`, rutas elegidas) congelada en el digest; CLI/RPC | Pruebas: renombrado en el archivo, sin copia → no se libera, conservados intactos, cambio tras revisar → rechazo, Papelera + journal. Corrida real: el usuario conecta MEDIA500/501 y revisa el plan | **Código hecho 9 nov** ([detalle](#m1--liberar-una-colección-implementado-2026-11-09)); falta la corrida real |
| M2 · Carpeta viva | Copia incremental verificada SSD1/`~/Music` → MEDIA502 sin renombrar ni limpiar; bundles de Ableton completos; informe de referencias externas | Segunda corrida sólo copia lo nuevo; borrar en origen no borra en archivo | **Código hecho 9 nov** ([detalle](#m2--carpeta-viva-implementado-2026-11-09)); falta la primera corrida real |
| M3 · Triage de audio | Contrato `audio-triage-v1` con Clipper (niveles 0–1 primero); filtros en la revisión | Fixture de silencio, ruido, voz y música clasificado; nunca borra | Pendiente |
| M4 · Biblioteca | Deduplicar por hash y huella; reorganizar con curado mediante `music-library-v1` (mapa de rutas + ACK, `rekordbox.xml`) | Un lote movido sin perder cues ni playlists en curado/rekordbox | Pendiente |

## M1 · Liberar una colección (implementado 2026-11-09)

Código: [collection_release.py](../../media/collection_release.py), pruebas en [test_collection_release.py](../../tests/test_collection_release.py) (12, sólo fixtures). Servicio: `collection_release_plan`, `collection_release_apply`, `collection_release_plans`.

**Garantías:**
- **Planear no mueve nada.** Indexa los archivos por tamaño (sólo metadatos), hashea en la fuente sólo lo que tiene un candidato del mismo tamaño y confirma por SHA-256 contra el archivo, aunque la copia tenga otro nombre.
- **El plan clasifica cada archivo** en *se libera*, *se conserva* (`ableton_analysis`, `ableton_set`, `explicit`, `clipper`), *sin copia* (`no_same_size`, `no_identical_copy`) o *no es media* (no se toca). Queda guardado con su digest y un resumen por primera carpeta (año).
- **Aplicar exige el digest revisado.** Por cada archivo, justo antes de moverlo: identidad del disco fuente y del archivo, rehash de la copia en el archivo y de la fuente (hash y stamp iguales a la revisión), y luego un rename relativo a descriptor hacia `.Trashes/<uid>/nuun-<plan>/<ruta original>`. Nunca hace `unlink`.
- **Clipper:** consulta en vivo antes del primer movimiento y cada 60 s. Cualquier bloqueo (también un proyecto desconectado) o uso de estos archivos detiene la corrida sin mover nada más.
- **Journal** `intent`/`done` por archivo y un evento `collection_released` por movimiento. Si se interrumpe, reanudar el mismo plan termina sin mover dos veces; un `intent` se acepta si la copia en la Papelera tiene el hash revisado.
- **Material nuevo** que aparezca en la carpeta después de revisar no bloquea ni se toca.
- Se rechaza una fuente que se cruza con un volumen registrado o que está en el mismo disco físico que el archivo.

**Runbook para SSD1 (operación física, la pide el usuario):**

1. Conectar SSD1, MEDIA500, MEDIA501 y **ESSD** (los proyectos de Clipper viven ahí; sin él, aplicar se bloquea por «Proyecto Clipper no disponible»).
2. Registrar MEDIA500/501 como archivos si no lo están (Ajustes → Discos, o `nu9ve media volumes register …`; escribe la marca de nüün en esos discos).
3. Planear (lee y hashea; con SSD1 y los MEDIA puede tardar horas, según cuántos archivos tengan candidato):
   ```sh
   nu9ve media collection-release-plan --source /Volumes/SSD1/recordingslong \
     --archive MEDIA500 --archive MEDIA501 --archive MEDIA502 \
     --als-root /Volumes/SSD1/abletonlive --als-root ~/Music
   ```
   `--search ARCHIVO:carpeta` limita dónde buscar en un MEDIA y lo hace más rápido; `--keep 2024` conserva una carpeta entera.
4. Revisar el `summary` por año, `kept` y `uncovered`. Lo que no tenga copia se respalda primero (M2).
5. Aplicar: `nu9ve media collection-release-apply <plan_id> --digest <digest>`. Al terminar, el espacio vuelve **al vaciar la Papelera de SSD1**.

Sin interfaz en la app todavía: por ahora es CLI/servicio. Pendiente de M1: la corrida real y, si se usa seguido, una vista de revisión en Discos.

## M2 · Carpeta viva (implementado 2026-11-09)

Código: [living.py](../../media/living.py), pruebas en [test_living.py](../../tests/test_living.py) (11, sólo fixtures). Servicio: `living_plan`, `living_apply`, `living_collections`. Tabla aditiva `living_files` en el catálogo (esquema sigue en 1).

**Garantías:**
- **Una colección** = una carpeta de origen ↔ una carpeta de un archivo registrado, con las mismas rutas relativas. Dos colecciones no pueden compartir ni anidar carpeta en el mismo archivo.
- **Planear no copia nada.** Lo que tiene el mismo tamaño y mtime que la última copia verificada, y sigue en el archivo, cuenta como *sin cambios* y no se relee. El plan lista *nuevos*, *cambiados* y *ya no están en el origen* (se quedan en el archivo), y comprueba el espacio libre.
- **Copiar** usa Transfer: `.ncpartial`, SHA-256 de lo escrito y luego el nombre final, sin reemplazar nada. Si en el archivo ya hay bytes idénticos, se adoptan.
- **Nada se sobrescribe.** Si el archivo tiene bytes distintos en esa ruta, la versión anterior se mueve dentro del mismo archivo a `<carpeta>/_nuun/versions/<corrida>/<ruta>` antes de copiar. Ninguna versión respaldada se pierde.
- **Nunca toca el origen:** no renombra, no limpia y no propaga borrados.
- Un archivo que cambia **después de revisar** se salta y se reporta (`changed_since_review`); la siguiente corrida lo toma.
- Si se interrumpe, se reanuda el mismo plan: lo ya copiado se adopta por hash.
- `<carpeta>/_nuun/manifest.json` en el archivo repite el índice (ruta, tamaño, SHA-256) para reconstruirlo sin el catálogo.
- **Informe Ableton** (`ableton` en el plan): sets leídos, referencias absolutas a muestras fuera de la carpeta, cuáles faltan en disco y cuáles **no cubre ninguna colección** (`not_covered`, con bytes). Las referencias relativas antiguas no se adivinan.

**Colecciones recomendadas** (decisión del usuario pendiente; se pueden cambiar antes de la primera corrida):

| Origen | Carpeta en MEDIA502 | Por qué |
| --- | --- | --- |
| `/Volumes/SSD1/abletonlive` (241 GB, incluye packs) | `005_produccion/ssd1-abletonlive` | Una sola colección conserva juntas las rutas que usan los sets; caben con 2.1 TB libres |
| `~/Music/01_produccion` (42 GB) | `005_produccion/music-01_produccion` | Material ya ordenado por tipo |
| `~/Music/03_final` (9 GB) | `005_produccion/music-03_final` | Finales de producción |
| `~/Music/00_assets` (12 GB, 11 GB son plugins) | opcional | Instaladores; quizá no vale la pena |

**Runbook (operación física, la pide el usuario):**

```sh
nu9ve media living-plan --source /Volumes/SSD1/abletonlive --archive MEDIA502 --folder 005_produccion/ssd1-abletonlive --name "Ableton (SSD1)"
# revisar new/changed/gone_from_source, bytes, free y ableton.not_covered
nu9ve media living-apply <plan_id> --digest <digest>
nu9ve media living-collections
```

Repetir `living-plan` + `living-apply` cuando se quiera: la segunda vez sólo copia lo nuevo o cambiado. Lo que M1 deje *sin copia* se puede respaldar igual (por ejemplo, `recordingslong` como colección). Sin interfaz en la app todavía.

## Para retomar

Leer esta página → [inventario](music-ssd1-inventory-2026-11-09.md) → la fila de la fase en curso. El código dueño de M1 es [release.py](../../media/release.py). Los números del inventario salieron de listados de sólo nombre y tamaño; repetirlos no requiere abrir archivos (`find … -print0 | xargs -0 stat -f '%z\t%m\t%N'`). Las referencias de Ableton se leen descomprimiendo cada `.als` (gzip XML) y buscando rutas `.wav`.

## Orden propuesto (resumen)

1. **Liberar `recordingslong`/`recordings` contra MEDIA500/501** (sección 1). Es el mayor alivio (~250 GB) y reutiliza la liberación existente. Necesita conectar esos discos.
2. **Carpeta viva** para producción y `~/Music` hacia MEDIA502, con el informe de referencias externas.
3. **Triage de audio** con Clipper (`audio-triage-v1`), primero los niveles 0–1.
4. **Biblioteca:** respaldar `mix/` y `selec/` tal como están, después deduplicar y luego reorganizar con curado y el contrato `music-library-v1`.
