# Plan de mejoras

Este proyecto ya resuelve el flujo principal de respaldo, pero hay varias mejoras claras para hacerlo mas seguro y comodo en uso diario.

## Prioridad alta

- Agregar `--dry-run` a `save` para listar acciones sin tocar la SD.
- Agregar confirmacion antes de borrar fuentes cuando se corre contra una SD real.
- Separar configuracion local en un archivo ignorado por git, por ejemplo `config.local.json`, para no editar `constants.py` por maquina.
- Agregar tests para naming, rutas destino, deduplicacion y preservacion de fechas.
- Validar que el drive destino este montado antes de empezar a procesar archivos.
- Escribir un manifest por importacion con fuente, destino, fecha, tamano y checksum.

## Prioridad media

- Calcular checksum antes y despues de copiar archivos grandes.
- Agregar resumen final: copiados, movidos, borrados, ignorados y errores.
- Agregar soporte explicito para perfiles de camara: Sony a6400, GoPro, Canon, Nikon, DJI, Insta360.
- Normalizar tags de ciudad/pais y evitar caracteres conflictivos en nombres.
- Reemplazar prints restantes por logging estructurado.
- Hacer que audio, video e imagen usen una misma funcion de duracion/fecha/naming.
- Detectar sidecars adicionales como `.xmp` cuando existan.

## Prioridad baja

- Agregar comando `nu9ve inspect-sd` para mostrar estructura detectada antes de importar.
- Agregar comando `nu9ve eject` para expulsar la SD despues de una importacion exitosa.
- Crear reportes por importacion en Markdown o CSV.
- Revisar `download` para usar listas de argumentos seguras en vez de partir comandos por espacios.
- Documentar mejor `clipper` y mover ejemplos grandes a `clipper/README.md`.

## Decision actual sobre Sony XML

Conservar los XML en `metadata/sony/YYYY/MM` con el mismo nombre base del MP4 renombrado. No pesan casi nada y traen metadata que puede ser util despues: fecha de creacion, modelo de camara, codec, resolucion, fps y audio. Los thumbnails y archivos de estado de `M4ROOT` no deben borrarse automaticamente desde la computadora porque forman parte de la base de datos que la camara espera encontrar.

Actualizacion: no borrar automaticamente archivos de base de datos de Sony ni mover media Sony fuera de la tarjeta. La camara puede mostrar `Image Database File error` cuando se borran media o indices desde la computadora. El flujo mas estable es importar al backup en modo copy-only y formatear la tarjeta desde la camara cuando se quiera limpiar por completo.
