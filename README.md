# n-creator-utils

Utilidades personales para organizar archivos de un workflow creativo: respaldar tarjetas SD, ordenar fotos/video/audio por fecha y tipo, descargar material, separar stems y recortar videos.

## Requisitos

- macOS o Linux con Python 3.9+
- `ffmpeg` y `ffprobe`
- En macOS, `SetFile` ayuda a conservar fecha de creacion de filesystem al copiar entre volumenes
- Drives esperados por default:
  - SDs montadas en `/Volumes`
  - backup principal en `/Volumes/MEDIA502/001_raw`
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
nu9ve save "/Volumes/Untitled" mx hermosillo INSTA360
nu9ve save "/Users/nu9ve/Desktop/import" jp tokyo PHONE
```

`save all` revisa las tarjetas configuradas en `constants.py` dentro de `SD_SRC_NAME`. Antes de importar, el script detecta el perfil por estructura/contenido y puede sobreescribir el nombre configurado si encuentra una senal mas fuerte. Por ejemplo, una tarjeta llamada `EOS_DIGITAL` puede resolverse como `INSTA360` si contiene `.insv/.lrv/.thm/.insp` o `DCIM/fileinfo_list.list`. Una tarjeta llamada `Untitled` puede resolverse como `INSTA360` por esas mismas senales, o como `SONYA6400` si tiene estructura Sony `PRIVATE/M4ROOT` / AVCHD. Si una tarjeta `AUTO` no se reconoce, la salta.

Tipos soportados:

- Imagenes: `jpg`, `jpeg`, `png`, `cr2`, `cr3`, `arw`, `nef`, `dng`, `heic`, `gif`
- Video: `mov`, `mp4`, `mpg`, `mts`
- 360: `insv`, `lrv`, `thm`, `insp`
- Audio: `wav`, `mp3`
- Corruptos: `dat`

### Insta360

Insta360 no se guarda como archivos 360 sueltos renombrados. Cuando `src_name` es `INSTA360`, el script mueve la estructura original completa de la tarjeta a:

```text
/Volumes/MEDIA502/001_raw/insta360/YYYY/MM/DDMM-HHMMSS_COUNTRY-CITY_INSTA360/
```

Ejemplo:

```text
/Volumes/MEDIA502/001_raw/insta360/2026/06/1406-173422_MX-HERMOSILLO_INSTA360/
  DCIM/...
  MISC/...
  manifest.json
```

Este flujo libera la SD original, pero preserva nombres y carpetas para poder copiar despues el contenido de esa carpeta a otra SD e intentar editar desde la app/celular. Para rehidratar una SD, copia las carpetas originales internas como `DCIM` y `MISC` a la raiz de la SD; no copies la carpeta `1406-...` como contenedor.

Los `.INSV` son los archivos principales. Los `.LRV` son proxies de baja resolucion y los `.THM` thumbnails; no conviene renombrarlos ni separarlos si quieres que Insta360 Studio, la app movil o una tarjeta reconstruida los relacionen bien.

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
- Mueve MP4/JPG/ARW/MTS sin convertirlos y libera la SD.
- Mueve el XML sidecar a `metadata/sony/YYYY/MM` con el mismo nombre base del video.
- Tambien importa clips AVCHD `.MTS` si existen en `PRIVATE/AVCHD/BDMV/STREAM`.
- Archiva archivos de base de datos/cache de sesion como `AVF_INFO`, `MEDIAPRO.XML`, `STATUS.BIN`, thumbnails o indices AVCHD en `metadata/sony/support/YYYY/MM`, para no dejar la SD con entradas apuntando a media movida.
- Conserva la estructura base de carpetas Sony para seguir usando la SD en la camara; por ejemplo no intenta borrar `PRIVATE/SONY/SONYCARD.IND`.
- Conserva `mtime/atime` cuando el filesystem lo permite y, en macOS, intenta conservar/ajustar creation date con `SetFile`.

Los XML de Sony si sirven: guardan metadata de camara, codec, fps, resolucion y fecha real de creacion. Para un archivo cotidiano tal vez no los abras nunca, pero son baratos y utiles como sidecar de archivo maestro. Por eso el flujo los conserva en `metadata/sony` en vez de mezclarlos con los videos o tirarlos.

La Sony puede mostrar `Image Database File error` despues de mover/borrar desde la computadora porque su base de datos interna cambia fuera de la camara. Este flujo prioriza liberar la SD: mueve media, conserva sidecars y archiva caches/indices. Si la vas a volver a usar en la Sony y aparece aviso de base de datos, deja que la camara la reconstruya o formatea desde la camara despues de verificar el backup.

## Destinos

Los destinos se configuran en `constants.py`.

- Videos 1080p: `/Volumes/MEDIA502/001_raw/videos/YYYY/MM`
- Videos 4K: `/Volumes/MEDIA502/001_raw/v4k/YYYY/MM`
- Insta360 rehidratable: `/Volumes/MEDIA502/001_raw/insta360/YYYY/MM`
- Metadata sidecar: `/Volumes/MEDIA502/001_raw/metadata/sony/YYYY/MM`
- Sony support/cache archivado: `/Volumes/MEDIA502/001_raw/metadata/sony/support/YYYY/MM`
- Drone: `/Volumes/MEDIA502/001_raw/vdron/YYYY/MM`
- Imagenes: `/Volumes/MEDIA502/001_raw/images/YYYY/MM`
- RAW: `/Volumes/MEDIA502/001_raw/rimages/YYYY/MM`
- Screenshots: `/Volumes/MEDIA502/001_raw/screens/YYYY/MM`
- Audio: `/Volumes/MEDIA502/001_raw/recordings/YYYY/MM`

## Notas de seguridad

- Monta el drive de backup antes de correr `save`.
- Si el destino no existe o no tiene permisos, el script falla antes de borrar el archivo fuente.
- El movimiento entre volumenes se hace como copiar, restaurar fechas y borrar fuente.
- Las Insta360 se mueven como bundle intacto cuando usas `src_name` `INSTA360`; no se renombran por archivo.
- Las Sony a6400 mueven media y archivan caches/indices de sesion, pero conservan carpetas base de la camara. La camara podria pedir reconstruir base de datos al reinsertar.
- Si ya existe un archivo con el mismo nombre, se agrega `-02`, `-03`, etc.

## Desarrollo

Verificar sintaxis:

```sh
python3 -m py_compile nu9ve.py manager/file.py manager/video.py manager/audio.py constants.py
```

El proyecto todavia no tiene suite de tests automatizada. Para cambios en `save`, prueba primero con una copia temporal de una carpeta pequena de SD.
