# n-creator-utils

Utilidades personales para organizar archivos de un workflow creativo: respaldar tarjetas SD, ordenar fotos/video/audio por fecha y tipo, descargar material, separar stems y recortar videos.

## Requisitos

- macOS o Linux con Python 3.9+
- `ffmpeg` y `ffprobe`
- En macOS, `SetFile` ayuda a conservar fecha de creacion de filesystem al copiar entre volumenes
- Drives esperados por default:
  - SDs montadas en `/Volumes`
  - backup principal en `/Volumes/MOMMA/001_raw`
  - backup secundario de audio en `/Volumes/SSD1`

Instalacion basica en macOS:

```sh
brew install ffmpeg
```

Opcionalmente agrega un alias en `~/.zshrc`:

```sh
alias nu9ve='python3 /Users/nu9ve/code/nu9ve/n-creator-utils/nu9ve.py'
```

## Comandos

```sh
nu9ve save [src_dir|all|hijack] [country] [city] [src_name]
nu9ve download [video|song|sample] [url]
nu9ve split [file_or_url]
nu9ve clip [video_path]
```

### `save`

Organiza media en carpetas por tipo, ano y mes, usando nombres normalizados:

```text
YYMONDD-HHMMSS_SOURCE_[QUALITY_]DURATION_COUNTRY-CITY[].EXT
```

Ejemplos:

```sh
nu9ve save all mx hermosillo
nu9ve save "/Volumes/NO NAME" mx hermosillo SONYA6400
nu9ve save "/Users/nu9ve/Desktop/import" jp tokyo PHONE
```

`save all` revisa las tarjetas configuradas en `constants.py` dentro de `SD_SRC_NAME`.

Tipos soportados:

- Imagenes: `jpg`, `jpeg`, `png`, `cr2`, `cr3`, `nef`, `dng`, `heic`, `gif`
- Video: `mov`, `mp4`, `mpg`, `mts`
- 360: `insv`, `lrv`, `thm`, `insp`
- Audio: `wav`, `mp3`
- Corruptos: `dat`

### Sony a6400 / XAVC S

La Sony a6400 monta una estructura como:

```text
/Volumes/NO NAME
  DCIM/100MSDCF
  PRIVATE/M4ROOT/CLIP
  PRIVATE/M4ROOT/THMBNL
```

Los clips XAVC S viven en `PRIVATE/M4ROOT/CLIP` como pares:

```text
C0001.MP4
C0001M01.XML
```

El script ahora:

- Detecta clips dentro de `PRIVATE/M4ROOT/CLIP`.
- Usa `CreationDate` del XML como fuente principal para el naming.
- Si no hay XML, usa `creation_time` del MP4 via `ffprobe`.
- Lee resolucion y fps del XML para clasificar/diagnosticar, pero no ensucia el nombre con `1920x1080`.
- Copia MP4/JPG/MTS sin convertirlos y deja los originales en la SD.
- Copia el XML sidecar a `metadata/sony/YYYY/MM` con el mismo nombre base del video.
- Tambien importa clips AVCHD `.MTS` si existen en `PRIVATE/AVCHD/BDMV/STREAM`.
- No borra archivos de base de datos de la camara como `AVF_INFO`, `MEDIAPRO.XML`, `STATUS.BIN`, thumbnails o indices AVCHD.
- Conserva `mtime/atime` con `copy2`/`copystat` y, en macOS, intenta conservar/ajustar creation date con `SetFile`.

Los XML de Sony si sirven: guardan metadata de camara, codec, fps, resolucion y fecha real de creacion. Para un archivo cotidiano tal vez no los abras nunca, pero son baratos y utiles como sidecar de archivo maestro. Por eso el flujo los conserva en `metadata/sony` en vez de mezclarlos con los videos o tirarlos.

La Sony puede mostrar `Image Database File error` si se borran o mueven archivos de la tarjeta desde la computadora. Por eso los imports Sony son copy-only: respalda con `save`, verifica el backup y despues formatea la tarjeta desde la camara para dejarla limpia sin avisos.

## Destinos

Los destinos se configuran en `constants.py`.

- Videos 1080p: `/Volumes/MOMMA/001_raw/videos/YYYY/MM`
- Videos 4K: `/Volumes/MOMMA/001_raw/v4k/YYYY/MM`
- Metadata sidecar: `/Volumes/MOMMA/001_raw/metadata/sony/YYYY/MM`
- Drone: `/Volumes/MOMMA/001_raw/vdron/YYYY/MM`
- Imagenes: `/Volumes/MOMMA/001_raw/images/YYYY/MM`
- RAW: `/Volumes/MOMMA/001_raw/rimages/YYYY/MM`
- Screenshots: `/Volumes/MOMMA/001_raw/screens/YYYY/MM`
- Audio: `/Volumes/MOMMA/001_raw/recordings/YYYY/MM`

## Notas de seguridad

- Monta el drive de backup antes de correr `save`.
- Si el destino no existe o no tiene permisos, el script falla antes de borrar el archivo fuente.
- El movimiento entre volumenes se hace como copiar, restaurar fechas y borrar fuente.
- Las Sony a6400 se copian sin borrar la SD; limpia la tarjeta formateando desde la camara.
- Si ya existe un archivo con el mismo nombre, se agrega `-02`, `-03`, etc.

## Desarrollo

Verificar sintaxis:

```sh
python3 -m py_compile nu9ve.py manager/file.py manager/video.py manager/audio.py constants.py
```

El proyecto todavia no tiene suite de tests automatizada. Para cambios en `save`, prueba primero con una copia temporal de una carpeta pequena de SD.
