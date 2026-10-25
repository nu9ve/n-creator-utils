# Clipper antiguo — reemplazado

**Este recortador fue sustituido por [nu9ve/clipper](https://github.com/nu9ve/clipper)**, cuyo checkout de trabajo está en `/Users/nu9ve/code/ai/clipper`. La edición, composición, subtítulos, previews y exports deben evolucionar allí. Stalker produce las capturas que ese editor importa.

El código de esta carpeta y `nu9ve clip` todavía existen, pero no son el flujo vigente. Las instrucciones siguientes se conservan sólo para interpretar trabajos antiguos mientras se retira el módulo en un cambio separado. Su `config.json` no es `clipper-project.json` y no hay conversión automática entre ellos.

La relación actual entre los tres repos se documenta en [manejo de archivos y flujos](../docs/file-workflows.md).

## Referencia histórica

Recorta un video por rangos y genera encuadres mediante FFmpeg. Se invoca desde el comando `nu9ve clip`; también funciona `python3 /ruta/al/repo/nu9ve.py clip ...`. `clipper/clipper.py` define la clase de trabajo, pero no tiene una entrada standalone propia.

## Inicio rápido

Requiere Python 3.9+, FFmpeg y FFprobe. Para títulos/textos hace falta el filtro `drawtext` y una fuente disponible. No requiere OpenAI ni Replicate para el procesamiento local.

```text
proyecto/
  video.mp4
  config.json
```

```json
{
  "clips": [
    {"title": "Primer clip", "start": "00:01:10", "end": "00:01:15"}
  ]
}
```

```sh
nu9ve clip "/ruta/proyecto/video.mp4" --preview
nu9ve clip "/ruta/proyecto/video.mp4" clips.json
nu9ve clip "/ruta/proyecto/video.mp4" "/otra/ruta/clips.json" --preview --verbose
```

El JSON predeterminado se busca junto al video. Un JSON explícito tiene prioridad; si su ruta es relativa, se resuelve respecto a la carpeta del video. Los recursos `logo` y `background` relativos se buscan respecto al JSON. No hay lector TXT.

`--preview` **crea primero un video intermedio por rango** y luego un JPEG por encuadre, tomando la imagen aproximadamente al segundo 1 del recorte. Usa rangos de al menos unos segundos para que exista ese frame. Escribe junto al video, necesita espacio y no es dry-run. Prueba estilos con preview antes de renderizar completos.

Los resultados usan el nombre fuente más `_clip_0.mp4`, `_clip_1.mp4`, etc.; las variantes añaden sufijos como `_vertical_background_90.mp4` o `.jpg`. El corte inicial usa stream copy, por lo que la precisión depende de keyframes; no se garantiza precisión por frame. La extensión de los intermedios es MP4; no todo codec de entrada puede copiarse a ese contenedor.

Las salidas existentes provocan error, sin pregunta interactiva ni sobrescritura. Para probar preview y luego render completo en el mismo directorio, conserva/reubica los intermedios previos antes de repetir. No hay directorio de salida o limpieza automática configurable.

## Campos implementados

| Campo | Ámbito/default | Comportamiento |
| --- | --- | --- |
| `clips` | Global, obligatorio | Array de trabajos; cada uno necesita `start` y `end`. |
| `start`, `end` | Por clip | Segundos numéricos o `HH:MM:SS[.fracción]`. Ambos son posiciones absolutas; `end` debe ser mayor. |
| `mode` | Global o por clip, `#101010` | Color de fondo; estilos históricos `crop_center`, `blurred`, `overlay`. El soporte varía por orientación/renderer. |
| `view` | Global o por clip, `90` | Valor entre 0 y 100 que controla el tamaño/recorte según el estilo; no es una escala uniforme en todos los formatos. |
| `vertical_only` | Global o por clip, `true` | Controla si además se generan cuadrado y versión con texto en orientación original; ver selección de formatos. |
| `texts` | Global y por clip, `[]` | Los locales se agregan después de los globales. Cada entrada lleva `text` y puede incluir `y` y `text_size`. |
| `title` | Por clip | Añade una entrada de texto con `y = 98 - view`. |
| `text_size` | Global, `3`, o por entrada de texto | Porcentaje de la altura; los tamaños mayores de 3 se reducen en 1 para textos largos. |
| `logo` | Global | Archivo adicional de imagen; escala/posición/opacidad se fijan en el renderer. |
| `background` | Global | Un color `#...` actúa como fondo sin abrir un archivo; una ruta añade un recurso para los estilos que lo usan. |

El esquema valida la estructura principal, rangos y algunos tipos; no es un validador completo de colores, recursos o filtros FFmpeg. Claves desconocidas pueden ignorarse. Los campos antiguos `top_text` y `bottom_text` de ejemplos full no se leen; usa `texts`.

Ejemplo con dos formatos adicionales, fondo y texto:

```json
{
  "mode": "#213233",
  "view": 80,
  "vertical_only": false,
  "text_size": 3,
  "texts": [{"text": "Mi proyecto", "y": 85}],
  "clips": [
    {"title": "Primer segmento", "start": "00:00:10.50", "end": "00:00:15.75"},
    {"title": "Segundo segmento", "start": 30, "end": 36, "view": 90}
  ]
}
```

## Selección de formatos

- Fuente horizontal (`ancho/alto > 1`): genera versión vertical.
- Fuente vertical o cuadrada: genera versión horizontal.
- Con `vertical_only: false`: además genera cuadrado y una versión de la orientación original con los filtros de texto.

El nombre histórico `vertical_only` es impreciso: no obliga a salida vertical para una fuente vertical. No hay selector general de formatos/resoluciones. Existe un renderer 3:4 en el código, pero la orquestación no lo invoca.

Para la fuente tipográfica configura `clipper_font_file` en el JSON local del repo o `NU9VE_CLIPPER_FONT_FILE`. Si el archivo no existe, se solicita `Sans` al sistema. Esto puede cambiar la apariencia respecto al antiguo archivo Coolvetica personal. `doctor` muestra si existe la fuente configurada.

## IA experimental

`--openai` conserva un experimento de transcripción con Replicate y selección de rangos usando llamadas OpenAI heredadas. Necesita SDKs compatibles, `REPLICATE_API_TOKEN` y `OPENAI_API_KEY`. Puede enviar audio y transcripción a esos servicios; el núcleo local no los contacta.

Las referencias de modelo están fijadas en código, se consideran sólo los primeros 40 segmentos y se guarda cache `segments_*.p` en el directorio de ejecución. **No cargues pickle ajeno**: no es un formato seguro para datos sin confianza. El modelo, SDK, formato de respuesta y calidad del resultado requieren migración/validación antes de recomendar este flujo. No se ejecutaron llamadas externas en esta revisión.

El archivo `openai_clipper.py` es otro experimento standalone y mantiene un audio `./lower.mp3` fijo; no es necesario para este comando. La generación de IA no guarda un JSON de configuración listo para reutilizar, aunque un log histórico diga “Generating config.json”.

## Límites y siguientes mejoras

- El escape de texto para `drawtext` aún no cubre bien comillas, dos puntos, barras y otros caracteres especiales. Valida esos casos con preview.
- Logos y fondos multimedia, especialmente ambos juntos, usan ramas históricas y necesitan pruebas específicas por orientación/estilo. Su presencia en el esquema no garantiza todas las combinaciones.
- Quedan dimensiones, curvas de `view`, colores y posiciones de logo dentro de `ffmpeg.py`; no hay presets extensibles.
- Las invocaciones fallidas de FFmpeg detienen el flujo; los outputs previos pueden permanecer. No hay rollback/reanudación ni limpieza automática.
- `debug: true` en la configuración del repo omite ejecución FFmpeg y sólo registra comandos con nivel debug; no garantiza salidas válidas y no es simulación del importador.

[Configuración de ambiente](../docs/configuration.md) · [Validación](../docs/development.md) · [Plan de mejoras](../PLAN.md)
