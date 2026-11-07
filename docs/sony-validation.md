# Validación Sony: respaldo, limpieza y formato en cámara

Estado actual: con una α6400 y la build `a439bb378645`, la primera retirada pareció terminar con la SD montada y los archivos reaparecieron tras reconectarla. La repetición controlada con los mismos tres archivos persistió al desmontar y montar la tarjeta en macOS. Falta comprobar el formato posterior y el cierre de ese job. [Prueba del 7 de noviembre](#prueba-física-con-un-clip--2026-11-07).

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


## Aceptación acompañada Sony y dos multicams · 2026-11-06

**Preparada, no ejecutada.** El usuario reporta Untitled formateada/reutilizada varias veces y dos videos multicam recién grabados para publicar. El usuario elige nombres, perfil y fuentes desde nüün; no preconfigurar ni preparar planes por CLI. No equiparar dos multicams con dos clips ni asumir que ambos son un proyecto.

1. **Inicio:** cerrar sólo la ventana, mantener nüün activo en menubar. Destinos registrados MEDIA y SSD de trabajo conectados; popup activado. Anotar build efectiva. No salir de la app ni instalar/reiniciar durante un job.
2. **Montaje:** insertar SD. Debe ofrecer Ingreso aunque haya resultado previo, sin copiar/limpiar ni iniciar lectura profunda por el montaje. Sin destinos online debe permanecer silencioso. Registrar qué ventana/mensaje aparece; si no hay popup, abrir desde menubar sin reinsertar repetidamente para forzarlo.
3. **Entrada:** inspeccionar explícitamente. Revisar todos los clips, proyectos y perfil por contenido; el proyecto general sólo rellena faltantes. Mantener auxiliares y pares. Sesiones Stalker deben estar terminadas; varias raíces aún requieren planes separados. Asignar el mismo proyecto a las fuentes de cada multicam, sin inventar sincronización desde nüün.
4. **Flujo:** revisar conteos/bytes, rutas MEDIA/SSD y política de limpieza. Sony debe indicar retirada de clips verificados e índices conservados para formato en cámara. Preparar plan no escribe media; sólo el botón final aplica el plan mostrado. Si falta SSD, decidir explícitamente pending/skipped o conectarlo. Revisar el plan concreto con el usuario antes de continuar.
5. **Resultado:** observar fases, verificación por destino y progreso global. En Sony, 100 % automático + `camera_action_required` es compatible: no significa que falló el respaldo ni que los clips sigan presentes. Si aparece «no se borró», guardar texto exacto/job y comprobar si es otro estado, copia conservada o job antiguo; no asumir éxito ni fallo por el aviso solo.
6. **Cámara:** después de verificar el resultado, expulsión manual; formato desde Sony. Volver a insertar y usar Actividad → «Ya la formateé…», eligiendo raíz de esa tarjeta. Esa acción sólo comprueba/cierra el registro; no formatea ni vuelve a borrar. Debe pasar a `completed` si no quedan clips originales y cambió algún índice registrado. No confirmar trabajos antiguos con otra tarjeta ni cerrar todos por el nombre Untitled.
7. **Edición:** entregar/importar en Clipper desde biblioteca, comprobar ambos multicams, sesiones/pistas y rutas de trabajo. Sync/colas son de Clipper; no reescribir JSON. Nüün no sincroniza multicam ni modifica una biblioteca Final Cut.
8. **Publicación y cierre, cuando proceda:** vincular finales publicados; «Respaldar publicados…» mueve sólo esos finales y conserva raw/render. Al declarar «ya no habrá más», revisar cierre completo: rehash, deps/ACK, confirmación de descartes y retiro journalizado. El editable permanece; retiro editorial se decide desde Clipper.

Registrar por etapa: fecha/build, proyecto/job, resultado observado y siguiente acción. Todos los pasos siguen **pendientes** hasta observarlos; no se infiere aceptación por reutilizar la SD. Desconexión/replay deliberados se prueban aparte con material ya resguardado.

**Aviso Sony recurrente:** por diseño actual cada operación Sony requiere formato/confirmación; no hay detección automática al reutilizar SD. UX a evaluar: distinguir respaldo/retirada completados de acción de cámara y evitar recordatorios obsoletos. No se ha decidido ni implementado eliminar el formato o la validación de índices. El usuario reporta formatos previos, no una confirmación técnica de cada job histórico.


### Resultado parcial reportado · 2026-11-06

Flotante aceptado; popup/reconexión no concluyente y pendiente. Usuario completó sus ingresos y necesita editar. Stalker completado; Sony mostró FileNotFoundError de C0001.MP4 y se conserva failed: el journal registra seis retiradas y recibos de archivo, sin causa confirmada ni evidencia de doble borrado. Dos clips Sony entregados después desde archivo a ESSD con herramienta existente, por solicitud explícita; esto no cierra el job ni valida formato. [Evidencia y siguiente diagnóstico](STATUS.md). No exigir otro ciclo físico ahora ni reanudar limpieza a ciegas.

## Prueba física con un clip · 2026-11-07

El usuario reportó apertura del compacto al insertar Untitled. Grabó un clip
de 9 s en una Sony ILCE-6400 y autorizó explícitamente el plan
`a7839591-f8b0-5583-9f7f-6ea107d168dc`, proyecto `prueba-sony-261107`.
El CLI del bundle instalado `a439bb378645` aplicó el mismo motor de nüün.

- MEDIA502 recibió 12 archivos (105,985,867 bytes). ESSD recibió el clip
  (33,563,329 bytes). Los 13 SHA-256 de destino coinciden con los originales
  leídos antes de ejecutar.
- Tras `backup_verified` se retiraron el MP4, el XML y la miniatura de la SD.
  Los tres constan como `cleaned=1` y sus rutas ya no existen. Los nueve
  índices conservan sus hashes.
- El job quedó `camera_action_required`, sin error y con avance global 1.0.
  Transcurrieron 13.3 s entre los eventos `copying` y `camera_action_required`.
  Esa observación sólo cubría la SD antes de expulsarla.

Al reconectar, el MP4, el XML y la miniatura reaparecieron con los mismos
hashes, tamaños y stamps. Los 13 destinos seguían íntegros. El registro de
macOS muestra que la primera expulsión desde Finder tomó unos 70 s, con
una interrupción de la conexión de FSKit durante el desmontaje. La causa
precisa sigue abierta. En una imagen ExFAT temporal con FSKit, el mismo
retiro con `Tree.unlink` persistió tras volver a montar.

En la SD real se revalidaron identidad, inventario y los 13 SHA-256 antes de
repetir únicamente el retiro de los mismos tres archivos autorizados. El
desmontaje normal tomó 0.41 s. Tras montar de nuevo en el mismo lector, los
tres seguían ausentes y los nueve índices coincidían byte a byte. La prueba
controlada usó el retiro de archivos del motor y conservó el job original en
`camera_action_required`. Falta reproducir la durabilidad como parte del
flujo normal de la app.

El usuario confirmó que la cámara formateó la SD correctamente después
del retiro. Se volvieron a verificar las 13 copias. La SD estaba fuera del
Mac al comprobar el catálogo. Falta reconectarla para comprobar la reescritura
de índices y confirmar el formato del job en Actividad.
