# Comandos

La referencia operativa vigente está en [operations](operations.md): registro de discos, safe save, plan/apply, jobs, catálogo, restore, pendientes, app y MCP. Cada entrada tiene `--help`.

## Cambio de entrada

- `nu9ve save FUENTE --project ...`: nuevo flujo revisable/verificado. Atajo: `save FUENTE PAIS PROYECTO`; el último texto es proyecto, no ciudad. `--no-work` elige sólo respaldo con trabajo omitido y confirmación por operación; ver [operations](operations.md#sólo-respaldo-sin-archivos-de-trabajo).
- `nu9ve media ...`: operaciones JSON compartidas por CLI y app. Respuesta `{ok, command, data}`; errores estructurados. El plan exportado puede ser crudo o envoltorio al aplicar.
- `nu9ve legacy-save DIRECTORIO PAIS CIUDAD PERFIL`, `legacy-save all PAIS CIUDAD` y `legacy-save hijack PAIS CIUDAD`: motor histórico. Mueve originales después de comparar tamaño y elimina algunos auxiliares; no tiene journal/checksums. [Detalle](importing.md).
- `nu9ve inspect DIRECTORIO`: inventario read-only `nu9ve-inspection/1`; no equivale a plan o verificación.
- `nu9ve doctor`: configuración/ejecutables/directorios del legado, no estado completo del catálogo ni certificación de discos.
- `nu9ve clip`: retirado; informa [sucesor](https://github.com/nu9ve/clipper) y termina con código 2.

## Descargas y stems

```sh
nu9ve download video "https://www.youtube.com/watch?v=..."
nu9ve download song "https://www.youtube.com/watch?v=..."
nu9ve download sample "https://soundcloud.com/artista/pista"
nu9ve split "/ruta/copia-audio.wav"
```

YouTube usa yt-dlp y el tipo elige destino/formato; SoundCloud usa scdl y depende de su salida para encontrar el archivo. URLs se pasan como argumentos separados. No se ha completado una integración de playlists/múltiples archivos ni el registro automático de provenance en el catálogo nuevo. Otros proveedores reconocidos pueden devolver no implementado.

`split` ejecuta Demucs, normaliza el nombre y mueve el original a `audio_splits_dir/splitted/` tras éxito. Puede romper referencias existentes: usar copia si el audio ya está referenciado. Modelos, GPU y flujo de derivados/caché versionada siguen pendientes. Estas herramientas conservan [configuración propia](configuration.md).

## Errores y salida

Argparse usa código 2 por sintaxis; éxito 0, fallo operativo 1 en el boundary JSON. `save` humano puede imprimir resumen/pedir confirmación; para automatización usar `media plan/apply` con digest. `--yes` no sustituye la confirmación independiente de archive-only.

`media handoff` imprime el contrato dentro de `data`; la exportación nativa escribe el contrato crudo. El consumidor debe distinguir ambos explícitamente, no interpretar la respuesta de error como un handoff. El esquema sólo valida forma: también hay que comprobar hashes, ubicaciones y recibos.

Diagnóstico, síntomas y cancelación/reanudación: [operations](operations.md#debug-aislado). No ocultar un error de identidad/espacio/checksum recurriendo al importador histórico.
