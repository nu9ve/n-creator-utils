# Configuración del legado y herramientas externas

El nuevo `save`/`media` usa catálogo, volúmenes registrados y defaults por servicio; ver [operations](operations.md). Este JSON sigue atendiendo legacy-save/download/split/doctor. El mapa `sd_src_name` también ayuda a `save all` a encontrar tarjetas, pero no registra destinos nuevos.

Esta configuración pertenece a `n-creator-utils`. No configura el editor externo ni Stalker. El editor interno fue retirado. `clipper_font_file` y `debug` en un JSON antiguo se ignoran por compatibilidad; sus variables de entorno ya no tienen efecto. Las rutas y ajustes de los otros repos se describen en [manejo de archivos y flujos](file-workflows.md#ubicaciones-y-configuración-actuales).

## Fuentes y precedencia

`configuration.load_settings()` carga, de menor a mayor prioridad:

1. Valores históricos del proyecto.
2. `config.local.json` junto a `configuration.py`, si existe, o el JSON seleccionado con `NU9VE_CONFIG`.
3. Variables de entorno `NU9VE_<CLAVE_EN_MAYÚSCULAS>` para las claves escalares.

La configuración se lee una vez al importar `constants.py`. Cambia las variables antes de iniciar el proceso; no hay recarga en caliente. El cargador también puede utilizarse directamente con una ruta, sin importar el CLI.

Un JSON parcial es suficiente. Las claves desconocidas, tipos inválidos y un `NU9VE_CONFIG` explícito que no exista producen error. Las rutas admiten `~` y variables de entorno; las variables deben estar definidas en el proceso. Las rutas relativas del JSON se resuelven respecto a ese JSON; las rutas relativas de variables `NU9VE_*`, respecto al directorio de ejecución.

`sd_src_name` se reemplaza completo con el mapa proporcionado; no se fusiona con el predeterminado y no tiene una variable de entorno equivalente. Cada clave debe ser un nombre de volumen simple, sin separadores de ruta.

```json
{
  "root_dir": "/mnt/media/001_raw",
  "sd_root_dir": "/media/miusuario",
  "root_recordings_dir": "/mnt/audio-backup",
  "sd_src_name": {
    "CAMERA": "AUTO",
    "RECORDER": "H6"
  },
  "yt_dlp_cookies_browser": ""
}
```

```sh
export NU9VE_CONFIG="$HOME/.config/n-creator-utils/config.json"
export NU9VE_AUDIO_SONGS_DIR="$HOME/Music/imported"
nu9ve doctor
```

El archivo indicado debe existir. `doctor` devuelve JSON con rutas efectivas, existencia de raíces, fuente y ubicación de ejecutables. El JSON puede incluir rutas personales; revísalo antes de compartirlo. `doctor` termina correctamente aunque falten herramientas: es un inventario, no una certificación de que todos los comandos puedan ejecutarse.

## Referencia

`~` significa el home real obtenido con `Path.home()`. `YYYY` se calcula al iniciar el proceso.

| Clave JSON | Valor predeterminado | Uso |
| --- | --- | --- |
| `root_dir` | `/Volumes/MEDIA502/001_raw` | Raíz principal de importación. Debe existir antes de `legacy-save`. |
| `sd_root_dir` | `/Volumes` | Directorio donde `save all` busca nombres del mapa de tarjetas. |
| `root_recordings_dir` | `/Volumes/SSD1` | Raíz de segunda copia de audio; debe existir para importar audio. |
| `audio_hijack_dir` | `~/Music/Audio Hijack` | Fuente del alias `legacy-save hijack`. |
| `audio_samples_dir` | `~/Music/01_produccion/samples/YYYY/raw` | Destino de `download sample`. |
| `audio_splits_dir` | `~/Music/01_produccion/splits` | Directorio de trabajo de Demucs, descargas para split y originales procesados. |
| `audio_songs_dir` | `~/Music/02_mix/latest` | Destino de `download song`. |
| `tmp_video_dir` | `~/data/projectIX/raw` | Destino de `download video`. |
| `yt_dlp_cookies_browser` | `brave` | Valor de `--cookies-from-browser` para canciones/samples de YouTube. `""` desactiva esa opción. No se añade a video. |
| `sd_src_name` | Mapa en `configuration.py` | Nombre de volumen → perfil/etiqueta. `AUTO` intenta Sony/Insta360 y omite tarjetas desconocidas. |

Todas las filas excepto `sd_src_name` aceptan su variable de entorno, por ejemplo `NU9VE_ROOT_DIR` o `NU9VE_AUDIO_SONGS_DIR`. Los destinos por categoría (`images`, `v4k`, `metadata/sony`, etc.) se derivan de las raíces y mantienen sus nombres históricos; aún no son reglas configurables.

## Credenciales

Las integraciones leen credenciales exclusivamente del entorno o de argumentos de la función de descarga:

| Variable | Integración |
| --- | --- |
| `SOUNDCLOUD_CLIENT_ID` | `scdl`, si la instalación/cuenta necesita client ID. |
| `SOUNDCLOUD_AUTH_TOKEN` | `scdl`, si necesita autenticación. |

Los archivos `.env` están ignorados, pero **no se cargan automáticamente**. Exporta las variables en el shell o usa el gestor de secretos de tu ambiente. `doctor` no incluye estas variables.

El repositorio contenía valores incrustados de OpenAI, Replicate y SoundCloud. Se retiraron del código de trabajo; pueden seguir en el historial de Git. El propietario debe revocarlos y generar reemplazos. El cambio local no revoca credenciales ni reescribe el historial.

## Migración desde `constants.py`

Los nombres públicos de constantes y las carpetas de destino se conservan para los importadores existentes. Mueve cualquier personalización de rutas de tu copia anterior al JSON. Las rutas personales ahora dependen del home, no del directorio desde el que ejecutas el programa. El nombre `DRIVE_NAME` se conserva por compatibilidad pero no gobierna el destino: usa `root_dir`.

Antes de reutilizar estos módulos en otro proceso, establece `NU9VE_CONFIG` y después importa los módulos. Inyectar un objeto de configuración por operación es una mejora pendiente; hoy las constantes importadas son globales.
