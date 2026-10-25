# Instalación base y utilidades históricas

Para motor nuevo, app, runtime, registro de discos y debug usa [operations](operations.md); para distribución [release](release.md). Los resultados de pruebas citados abajo pertenecen al snapshot anterior; la evidencia actual está en [STATUS](STATUS.md).

Estas instrucciones instalan las utilidades de organización/archivo de este repo. El editor vigente se instala desde [nu9ve/clipper](https://github.com/nu9ve/clipper); el editor interno fue retirado junto con sus opciones y dependencias de IA.

## Núcleo local

Necesitas Python 3.9+ y los ejecutables `ffmpeg` y `ffprobe` en `PATH`. `save` necesita `ffprobe` para video y MP3; las imágenes y WAV PCM se inspeccionan con la biblioteca estándar. `inspect` no requiere ejecutables externos. Las pruebas actuales se ejecutaron con Python 3.9.6 y 3.12.12 en macOS; no constituyen una matriz completa de compatibilidad entre versiones/plataformas.

```sh
# macOS
brew install ffmpeg

# Debian/Ubuntu
# sudo apt install python3 python3-venv ffmpeg

python3 -m venv .venv
source .venv/bin/activate
python -m pip install --upgrade pip
python -m pip install -e .
nu9ve --help
nu9ve doctor
```

La instalación editable mantiene el comando `nu9ve` enlazado al checkout. Reactiva el entorno en una terminal nueva. El entorno y la instalación de FFmpeg son independientes. El backend de empaquetado usa setuptools; una primera instalación puede necesitar acceso al índice de paquetes.

Sin instalar el paquete:

```sh
python3 /ruta/al/repo/nu9ve.py --help
python3 /ruta/al/repo/nu9ve.py doctor
```

Así tampoco necesitas un alias con el nombre de otro usuario. Los dos modos usan los mismos comandos y configuración. También se construyeron e instalaron wheels normal y editable en entornos temporales de Python 3.9, verificando el entry point desde fuera del checkout. Para un wheel normal, selecciona tu configuración con `NU9VE_CONFIG`; el archivo de ejemplo y la documentación están en el checkout.

## Herramientas por función

| Función | Dependencia adicional | Observación |
| --- | --- | --- |
| Fechas de creación en macOS | `SetFile` opcional | Se utiliza si está en `PATH`; fuera de macOS se conservan las fechas que permita el filesystem. |
| YouTube | Ejecutable `yt-dlp`; FFmpeg para extracción/mezcla | Puede instalarse en el entorno con `python -m pip install yt-dlp`. Cookies opcionales según proveedor. |
| SoundCloud | Ejecutable `scdl` | Puede instalarse con `python -m pip install scdl`; opciones y formato de salida dependen de su versión. |
| Stems | Ejecutable `demucs` | Puede instalarse con `python -m pip install demucs`. Sus dependencias de audio/ML y modelos deben validarse en un entorno compatible. |
| Prototipos MIDI | `rtmidi`, `novation_launchpad.charset` y dispositivo compatible | El repo no incluye `charset` ni un flujo CLI conectado. Requiere trabajo de integración. |

No existe un lockfile de herramientas externas ni se garantiza que su última versión sea compatible. Las pruebas de descargas y Demucs simulan sus procesos; para adoptarlas en producción hay que fijar y probar versiones en el ambiente destino. Para evitar conflictos de ML, se pueden instalar herramientas en entornos separados siempre que sus ejecutables estén accesibles en `PATH`.

## Configuración inicial

Copia [config.example.json](../config.example.json) a `config.local.json` en la raíz del checkout y edítalo. Si instalas fuera del checkout, usa `NU9VE_CONFIG` con una ruta explícita. Los otros repos tienen configuración propia.

Ejemplo para comenzar con destinos locales usando el archivo de ejemplo:

```sh
mkdir -p "$HOME/Media/archive" "$HOME/Media/audio-backup"
nu9ve doctor
```

Crea esos directorios sólo si realmente quieres almacenar allí. Para discos externos, monta el disco y revisa las rutas antes de crear carpetas. La comprobación de existencia no distingue un disco montado de una carpeta vacía del sistema.

## Plataformas

- **macOS:** diseño original del proyecto, `/Volumes` y fechas opcionales con `SetFile`.
- **Linux:** rutas configurables, por ejemplo `/media/usuario/TARJETA` o `/mnt/archive`. No hay gestión automática de montaje/expulsión; falta validación integral en Linux.
- **Windows:** no hay compatibilidad comprobada. Algunos nombres generados contienen `[]`, los filtros de FFmpeg tienen reglas propias de escape y quedan operaciones históricas basadas en `/`. Configurar rutas no demuestra soporte completo.

La carpeta temporal de descargas suele ser el directorio desde el que ejecutas el comando; asegúrate de tener espacio también allí. `split` guarda resultados bajo `audio_splits_dir`.
