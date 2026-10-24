# Comandos y problemas frecuentes

Los ejemplos usan el comando instalado `nu9ve`. Desde el checkout puedes sustituirlo por `python3 nu9ve.py`. Cada comando admite `--help`.

## Importar

```sh
nu9ve save DIRECTORIO PAIS CIUDAD PERFIL
nu9ve save all PAIS CIUDAD
nu9ve save hijack PAIS CIUDAD
```

`PERFIL` es obligatorio para un directorio; puede ser `AUTO`, `SONYA6400`, `INSTA360`, `H6` u otra etiqueta. `all` busca sólo los nombres de volumen configurados. Una etiqueta arbitraria no incorpora un perfil nuevo con reglas propias.

`save all` resuelve perfiles por contenido; la detección puede sustituir el perfil del mapa. Con un directorio explícito sólo se detecta automáticamente si pides `AUTO`. En la detección actual Sony tiene prioridad sobre Insta360 si aparecen ambas estructuras. Las tarjetas `AUTO` desconocidas se omiten. Si ninguna tarjeta configurada existe, `all` termina sin importar.

**Este comando mueve y elimina archivos.** Consulta [importación](importing.md) para extensiones, rutas, Sony e Insta360. El proceso no es transaccional: si falla a mitad, algunos archivos ya pueden estar importados. No hay opción de simulación ni resumen estructurado por ejecución.

## Recortar video

```sh
nu9ve clip VIDEO [JSON] [--preview] [--verbose] [--openai]
```

El JSON predeterminado es `config.json` en la carpeta del video. Un JSON relativo también se busca en esa carpeta; una ruta absoluta se usa directamente. La opción explícita tiene prioridad. Logo y fondo relativo se buscan junto al JSON. [Guía completa del clipper](../clipper/README.md).

`--preview` realiza primero el corte del rango, luego extrae imágenes de los encuadres. `--verbose` activa mensajes de diagnóstico. `--openai` utiliza un prototipo heredado con dependencias adicionales y puede enviar audio/transcripción a proveedores externos; no es necesario para el recorte local.

## Descargas

```sh
nu9ve download video "https://www.youtube.com/watch?v=..."
nu9ve download song "https://www.youtube.com/watch?v=..."
nu9ve download sample "https://soundcloud.com/artista/pista"
```

| Tipo | Destino | YouTube |
| --- | --- | --- |
| `video` | `tmp_video_dir` | Solicita video MP4 y audio M4A, con fallback MP4. |
| `song` | `audio_songs_dir` | Extrae audio MP3; usa cookies del navegador configurado si no está vacío. |
| `sample` | `audio_samples_dir` | Igual que `song`, con destino distinto. |

SoundCloud usa `scdl` y su formato por defecto; `song`/`sample`/`video` eligen destino, no convierten automáticamente el resultado de SoundCloud a MP3/video. Los directorios de destino se crean cuando se mueve la descarga. Los archivos se descargan inicialmente en el directorio de ejecución, que debe ser escribible.

La URL se entrega como un argumento, preservando `&` y espacios. El servicio se detecta por hostname. Se obtiene la ruta de YouTube con `--print after_move:filepath`; SoundCloud todavía depende de reconocer una línea `Downloaded.` en la salida de `scdl`. Las colisiones en el destino reciben `-02`, `-03`, etc.

Spotify, Bandcamp, Mixcloud, Vimeo, Twitch, Facebook, Instagram, X y TikTok se reconocen por nombre, pero devuelven error de integración no implementada. No se validaron descargas reales en esta revisión. Usa URLs de un solo recurso: playlists, álbumes, múltiples archivos y reanudación no tienen un flujo de organización completo. Si la herramienta descargó algo y después falló el parseo o traslado, revisa el directorio de ejecución antes de repetir.

## Stems

```sh
nu9ve split "/ruta/cancion.wav"
nu9ve split "https://www.youtube.com/watch?v=..."
```

1. Para URLs usa el mismo adaptador de descarga en `audio_splits_dir`; un archivo local se resuelve desde el directorio actual.
2. Elimina espacios del nombre y guiones bajos antes del primer `[`; puede **renombrar el archivo local original**. Evita sobrescribir un nombre existente.
3. Ejecuta `demucs ARCHIVO` con `audio_splits_dir` como directorio de trabajo. Los stems siguen la estructura que cree Demucs, normalmente `separated/...`.
4. Sólo tras éxito mueve el audio original normalizado a `audio_splits_dir/splitted/`, creando esa carpeta si falta.

Si Demucs falla, el audio normalizado permanece donde estaba antes del paso 3. No hay rollback del renombrado inicial. El proceso Python no cambia su directorio de trabajo global. No se seleccionan modelo, GPU, cantidad de stems ni flags desde el CLI: hoy son los valores por defecto de Demucs.

## Diagnóstico y automatización

```sh
nu9ve doctor
nu9ve doctor > diagnostico.json
```

Salida JSON: `settings`, `executables`, `directories`, `clipper_font_exists`. No crea directorios ni procesa media. `--help` también funciona sin los SDKs opcionales.

El CLI devuelve `0` al completar un comando, `1` en fallos operativos controlados y `2` por argumentos inválidos. Una detección `AUTO` omitida o `all` sin tarjetas no es un fallo operativo. Los errores inesperados de programación pueden producir traceback. `control` devuelve un error claro porque el prototipo no está conectado. El código Python admite `nu9ve.main([...])`; la documentación del contrato de librería está en [módulos](modules.md).

## Resolver problemas

| Síntoma | Revisión |
| --- | --- |
| Falta `nu9ve` | Activa `.venv`, instala editable o usa la ruta absoluta a `nu9ve.py`. |
| `Archive root must already exist` | Comprueba configuración y montaje; la raíz no se crea automáticamente. |
| Falta `ffprobe`, `yt-dlp`, `scdl` o `demucs` | Revisa `PATH` con `doctor`; el ejecutable puede estar en otro entorno. |
| `ffprobe could not read` | El archivo puede estar incompleto, corrupto o sin stream de video. Se detiene antes de mover ese archivo. |
| Fallo por salida FFmpeg existente | Conserva/reubica las salidas previas o elige otra carpeta de trabajo; no se sobrescriben. |
| FFmpeg rechaza texto/filtros | Comprueba `drawtext`, fuente, caracteres con escape y estilo; prueba primero un JSON simple. |
| SoundCloud descargó pero no reportó ruta | Revisa archivos del directorio de ejecución y salida de tu versión de `scdl`. |
| Sony muestra error de base de datos | El flujo mueve media e índices fuera de la tarjeta. Tras verificar el respaldo, reconstruye/formatea desde la cámara. |
| Configuración no se aplica | Revisa `NU9VE_CONFIG`, variables que prevalecen y reinicia el proceso Python si ya había importado constantes. |
