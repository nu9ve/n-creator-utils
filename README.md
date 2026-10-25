# n-creator-utils

Herramientas de línea de comandos para importar, organizar y archivar media de cámaras, descargar material y separar audio en stems. Es una colección de utilidades personales en proceso de hacerse reutilizable; no tiene interfaz gráfica ni servicio HTTP.

La edición de video vive en [Clipper](https://github.com/nu9ve/clipper), y la captura de pantalla en [Stalker](https://github.com/nu9ve/stalker). El directorio `clipper/` y `nu9ve clip` de este repo son una implementación histórica **reemplazada por ese editor**; siguen presentes en el código, pero no son el flujo vigente ni un área que debamos seguir desarrollando aquí.

Para conectar captura, archivo y edición, empieza por [manejo de archivos y flujos entre repos](docs/file-workflows.md): ubicaciones actuales, referencias, bundles y oportunidades de integración.

## Qué hay en este repo

| Área | Qué hace hoy | Estado y alcance |
| --- | --- | --- |
| `save` / `manager/` | Importa fotos, video y audio; organiza por fecha y origen; conserva metadata Sony y bundles Insta360 | Flujo principal. **Mueve archivos y elimina originales después de copiar**; también elimina ciertas extensiones auxiliares. [Detalles](docs/importing.md). |
| `clip` / `clipper/` | Recortador histórico con JSON y FFmpeg | Reemplazado por [Clipper](https://github.com/nu9ve/clipper). Código pendiente de retiro; [nota de transición](clipper/README.md). |
| `download` | Descarga YouTube con `yt-dlp` y SoundCloud con `scdl`; organiza por video, canción o sample | Integración con programas externos. Otros servicios sólo están identificados, sin implementar. [Uso](docs/commands.md#descargas). |
| `split` | Ejecuta Demucs desde un archivo o URL | Integración externa; normaliza el nombre y archiva el original tras éxito. [Uso](docs/commands.md#stems). |
| IA del clipper antiguo | Experimento de transcripción y selección de clips con Replicate/OpenAI | Histórico y reemplazado. Las mejoras editoriales/IA corresponden al repo Clipper. |
| `hardware/`, `utils/`, `playground/`, `social/` | Drivers/prototipos MIDI, utilidades antiguas, scripts Bash y una carpeta vacía | No conectados al flujo principal. Se conservan. [Inventario](docs/modules.md). |

El reemplazo del clipper fue confirmado por el propietario. Para las otras utilidades históricas todavía falta confirmar usos y sucesores antes de retirarlas. Stalker y Clipper intercambian sesiones; este repo todavía no tiene un adaptador para conservarlas o relocalizarlas como unidad.

## Instalar

Python 3.9 o superior y `ffmpeg`/`ffprobe` disponibles en `PATH`. El núcleo Python usa la biblioteca estándar. No necesitas instalar los SDK de IA ni los drivers MIDI para importar u organizar media.

```sh
# macOS
brew install ffmpeg

# Debian/Ubuntu, como alternativa
# sudo apt install python3 python3-venv ffmpeg

git clone https://github.com/nu9ve/n-creator-utils.git
cd n-creator-utils
python3 -m venv .venv
source .venv/bin/activate
python -m pip install --upgrade pip
python -m pip install -e .
nu9ve --help
```

También puedes usar `python3 nu9ve.py ...` desde el repo, sin instalar el paquete. Para rutas, dependencias opcionales y límites de compatibilidad consulta [instalación](docs/installation.md).

## Configurar tu ambiente

```sh
cp config.example.json config.local.json
# Edita config.local.json: destinos, tarjetas y cookies.
nu9ve doctor
```

El archivo local se ignora en Git. Sin él se conservan los destinos históricos: `/Volumes/MEDIA502/001_raw`, `/Volumes/SSD1` y tarjetas en `/Volumes`. Las rutas personales se calculan con el directorio de inicio del usuario real.

Antes de `save`, crea o monta el directorio raíz de archivo que configuraste; para audio también debe existir la raíz de respaldo secundario. Las subcarpetas se crean al importar. `doctor` muestra configuración efectiva y programas disponibles en JSON, sin modificar archivos. Detecta directorios existentes; no garantiza que un disco esté montado ni verifica su identidad.

Puedes seleccionar otro JSON y sobreescribir valores con variables:

```sh
export NU9VE_CONFIG="$HOME/.config/n-creator-utils/config.json"
export NU9VE_ROOT_DIR="$HOME/Media/archive"
nu9ve doctor
```

[Referencia completa de configuración y precedencia](docs/configuration.md).

## Usar

```sh
# Prueba primero sobre una copia pequeña de tus archivos.
nu9ve save "/ruta/a/copia-de-tarjeta" mx hermosillo AUTO
nu9ve save all mx hermosillo
nu9ve save hijack mx hermosillo

# Requieren herramientas opcionales.
nu9ve download song "https://www.youtube.com/watch?v=..."
nu9ve split "/ruta/audio.wav"
```

`save` no tiene `--dry-run`, rollback ni verificación por checksum. La copia se comprueba por tamaño antes de eliminar el original; eso no equivale a verificar integridad. Algunas extensiones (`hprj`, `bmp`, `gyro`, `aae`) se eliminan directamente en el importador genérico. Lee [el comportamiento exacto de importación](docs/importing.md) antes de usarlo sobre una tarjeta real.

Importa a Clipper las rutas definitivas de tus originales. No uses `save` genérico para reorganizar una carpeta `.stalker`, un proyecto Clipper o una entrega completa: el importador actual recorre archivos sueltos y puede romper sus relaciones. [Flujos y límites actuales](docs/file-workflows.md).

## Documentación

- [Instalación y compatibilidad](docs/installation.md)
- [Configuración por usuario y credenciales](docs/configuration.md)
- [Comandos, salidas y resolución de problemas](docs/commands.md)
- [Importación, naming y cámaras](docs/importing.md)
- [Manejo de archivos e integración con Stalker y Clipper](docs/file-workflows.md)
- [Clipper antiguo: reemplazo y referencia histórica](clipper/README.md)
- [Inventario, arquitectura y puntos de integración](docs/modules.md)
- [Desarrollo y validación](docs/development.md)
- [Pendientes priorizados y límites actuales](PLAN.md)

## Desarrollo

```sh
python3 -m unittest discover -s tests -v
python3 -m compileall -q nu9ve.py constants.py configuration.py manager clipper hardware utils
```

Las pruebas usan carpetas temporales, mocks para servicios externos y videos sintéticos para FFmpeg. No usan tarjetas reales ni cuentas. No se han validado descargas en vivo, servicios de IA, Demucs con modelos reales, hardware MIDI ni Windows. [Alcance de las pruebas](docs/development.md).
