# Validar Sony sin modificar la tarjeta desde el importador

Estado: protocolo para una próxima prueba acompañada. El nuevo `save` verifica copias y deja Sony en `camera_action_required`; el motor histórico de mover/borrar quedó en `legacy-save`. **La limpieza Sony automática todavía no está validada en cámara**. `inspect` es observación y no hace respaldo. No se modificó ninguna SD durante esta revisión.

## Hipótesis

El código mueve clips, sidecars y soporte de Sony, incluidos `AVF_INFO`, thumbnails, `MEDIAPRO.XML`, `STATUS.BIN` y partes AVCHD. Eso puede dejar desajustada la base que usa la cámara. La detección, además, llama SONYA6400 a cualquier estructura Sony reconocida; confirmar modelo y firmware reales antes de generalizar.

Sony documenta problemas de base de imágenes tras procesar archivos en computadora y una función de reconstrucción. Esa documentación respalda la hipótesis general, pero no demuestra la causa exacta de tu aviso ni que debamos manipular sus índices. Véase [Recover Image DB](https://helpguide.sony.net/ilc/1940/v1/en/contents/TP0002647439.html) y [soporte sobre archivos modificados/borrados en computadora](https://www.sony.com.au/electronics/support/e-mount-body-ilce-7-series/ilce-7m4/articles/00272485). Las rutas del menú dependen del modelo.

## Prueba propuesta del flujo respaldar → verificar → liberar

1. Conservar cualquier contenido actual con respaldo verificado. Registrar modelo/firmware de cámara, SD/filesystem, modo de video y mensaje exacto. No formatear material pendiente para obtener una muestra.
2. Si decides preparar una tarjeta limpia desde la cámara, registrar su árbol recién formateado. Grabar dos clips pequeños y una foto; insertar en Mac. Snapshot de nombres, tamaños, fechas y hashes, incluidos índices/sidecars; no necesitar lectura de videos largos para la primera prueba.
3. Probar primero en **copia temporal del árbol**. El nuevo planner debe incluir clips y soporte como unidad preservada y mostrar destinos/espacio, sin eliminación.
4. Ejecutar la futura copia verificada en la SD: estructura original completa a archivo, selección de trabajo al SSD, lectura SHA-256 en ambos destinos y recibos. Comprobar que árbol/hashes de fuente permanecen iguales. El OS puede crear archivos auxiliares: distinguir esos cambios de los hechos por la app; considerar pestaña de bloqueo/lector que respete sólo lectura.
5. Expulsar de forma segura. Reinsertar en cámara, reproducir clips existentes y grabar otro. Registrar si reaparece aviso **sin borrar ningún archivo desde Mac**.
6. Esta reinserción sin limpieza sólo aísla el diagnóstico: no es el resultado final buscado. Después, con archivo y copias requeridas verificados, probar la liberación desde cámara como referencia. Reinsertar, confirmar espacio libre y grabar/reproducir un clip nuevo. Si vuelve el aviso, capturar mensaje y snapshot antes/después de la acción en cámara; así distinguimos formato, archivos del OS y cambios de base. No borrar índices manualmente para experimentar sobre la única copia.
7. En copia temporal y luego con muestra ya respaldada, validar un perfil de limpieza automático por modelo/formato. Comparar contra la estructura de referencia de cámara: sin clips anteriores, índices consistentes, sin aviso y grabación nueva correcta. Sólo habilitar ese perfil como cierre automático de `save` después de aprobar esta prueba. Hasta entonces mostrar «respaldo verificado · liberar en cámara», sin marcar SD lista ni exigir conservarla llena indefinidamente.

La prueba completa requiere cámara reutilizable, originales conservados y verificados en destinos, recibo completo, reintento sin duplicados y SD libre de grabaciones anteriores con estructura/índices válidos. Copy-only es una fase diagnóstica; respaldo sin liberar la tarjeta no cierra el objetivo de `save`. Una fixture de XML/video sintético sólo valida software: no cierra la prueba de cámara.

## Lo que ya se puede inspeccionar

```sh
nu9ve inspect "/ruta/a/copia-de-la-SD"
```

Reporta nombres y evidencia Sony del sidecar si está disponible. No verifica hashes, no exporta todo el inventario de atributos de la SD ni certifica que la cámara aceptará su base de imágenes. Para comparar snapshots de la prueba se añadirá un modo de evidencia con hashes al motor, separado del inventario rápido.

## Tarjeta formateada en cámara (ILCE-6400, 2026-10-25)

Primer ciclo real completo: respaldo verificado (`camera_action_required`), formateo en la cámara y cierre con `media confirm-cleanup` → `completed`. Huellas antes/después en la sesión de trabajo; resumen:

| Elemento | Después del formateo |
| --- | --- |
| `PRIVATE/M4ROOT/CLIP/*`, `THMBNL/*` | Eliminados (antes 21 archivos). |
| `PRIVATE/M4ROOT/MEDIAPRO.XML` | Regenerado, 406 B: lista vacía, `mediaId` nuevo, `createdAt` del formateo. |
| `AVF_INFO/*`, `PRIVATE/AVCHD/BDMV/*` | Mismos tamaños, contenido nuevo. |
| `STATUS.BIN`, `PRIVATE/SONY/SONYCARD.IND` | Idénticos. |
| Volumen ExFAT | 76 MB usados; **mismo Volume UUID** que antes del formateo. |

Desde el 2026-10-25 (perfil `sony_clips_then_format`) el motor borra de la tarjeta los clips, las miniaturas y las fotos `DCIM` tras verificar y releer cada archivo, para que el formateo en cámara sea rápido. Los índices quedan intactos. La cámara puede pedir «recuperar base de datos» al insertarla; da igual, el paso siguiente es formatear. `confirm-cleanup` exige además que la cámara haya reescrito algún índice. **Pendiente de probar con la próxima tarjeta real.**

Conclusión: la cámara reescribe sus índices con identificadores propios. Imitarlo desde la computadora sería manipular índices por inferencia, así que **la limpieza Sony sigue siendo formatear en la cámara**. El motor la cierra comprobando que no queda ningún clip respaldado (misma ruta, tamaño y `mtime`). Un clip nuevo con nombre reutilizado no bloquea. Como el UUID no cambia, no sirve para detectar el formateo.
