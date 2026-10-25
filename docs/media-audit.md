# Auditoría de media y decisiones de diseño

Auditoría inicial: 24 de octubre de 2026, America/Hermosillo. La implementación posterior está en [STATUS](STATUS.md); esta página conserva hallazgos y contexto de origen. Esta revisión incorpora el flujo relatado por el propietario, lectura de código y una inspección superficial de los discos conectados. No ejecutó respaldos, limpiezas ni migraciones personales.

## Evidencia y límites

| Fuente | Revisión | Qué se leyó |
| --- | --- | --- |
| n-creator-utils | Checkout de esta entrega | README, PLAN, configuración, importadores, CLI, pruebas y scripts históricos. |
| Stalker | `46161c7` + cambios locales en `docs/clipper-contract.md` | README, contrato 1.1, `SessionManifest.swift`, `StalkerApp.swift`, `AppModel.swift`. Las notas locales del contrato se leyeron sin modificarlas. |
| Clipper | `e6981b1`; `.claude/` sin seguimiento | Integración Stalker, importador, relink en `projects.py`, contratos de app/services, documentación de perfiles. No se ejecutó el editor ni se modificaron proyectos. |
| Atlas | `ba60ffa` | README y workflow de inputs/derivados. No se ejecutó fotogrametría ni se auditó todo el pipeline. |
| Discos | Snapshot de esta sesión | Nombres de carpetas a dos niveles, espacio e identidad mediante `diskutil info -plist`. Sin recorrer ni hashear todo el archivo. |

Una guía de otro repo puede describir comportamiento antiguo. Los hallazgos de relink/importación se contrastaron con código actual. Los costos de CPU/RAM de Stalker publicados en su README no se midieron de nuevo.

## Tu flujo actual reconstruido

1. Sony/Canon/DJI/360/H6 llegan a SD. Una tarjeta puede contener varios proyectos y canales.
2. `save all` organiza y retira originales hacia MEDIA502. Audio exige además SSD1, donde trabajas.
3. Copias manualmente raws seleccionados a ESSD: `001_proyectos/e64_raw`, a veces planos y a veces por proyecto. También agregas screens y ahora bundles Stalker.
4. Pegas fuentes en orden en Clipper; agrupas/sincronizas, editas y exportas versiones a Downloads.
5. Pasas versiones elegidas a `001_proyectos/e64`, editas canales en paralelo en Final Cut y exportas master y verticales a `002_finales/e64`.
6. El master conserva fecha+título; verticales agregan `_vertical_N`. Shorts/thumbnails están en subcarpetas. Tras publicar, mueves seleccionados a `subidos` para archivar luego en MEDIA.
7. Alternas perfiles como erikiado64 y nu9ve xyz; Atlas comparte fotos de dron y necesita originales JPG+DNG, datos de vuelo y derivados.

La organización física es útil. Los problemas aparecen cuando una ruta o un nombre intenta expresar simultáneamente cámara, fecha, proyecto, perfil, estado de publicación y respaldo. Esos datos necesitan campos independientes y relaciones persistentes.

Aclaración del propietario: la SD debe terminar libre para grabar nuevos proyectos. La conservación de fuente propuesta es temporal durante copia/verificación, no una política de acumular grabaciones en la tarjeta. El éxito completo incluye limpieza y consistencia de cámara; no basta con un respaldo que deja pendiente liberar la SD.

Aclaración adicional: si está MEDIA pero falta ESSD/SSD1, permitir confirmar sólo respaldo y registrar trabajo pendiente u omitido para decidirlo después. Sin ningún destino registrado de archivo/trabajo conectado, insertar SD no debe disparar popup ni notificación: se presume uso de consulta ajeno al ingreso.

## Discos observados

| Volumen | Total / libre decimal aproximado | Filesystem | Uso y estructura confirmada |
| --- | --- | --- | --- |
| ESSD | 499.76 GB / 33.78 GB | HFS+ Journaled | `000_acomodar`, `000_allcam`, `000_cache`, `001_proyectos/{e64_raw,e64,atlas-subaereo,atlas-derived,...}`, `002_finales/{e64,nu9vexyz,...}`. |
| MEDIA502 | 5.00 TB / 2.71 TB | ExFAT | `000_acomodar`, `000_crudo`, `001_proyectos`, `001_raw/{360,images,insta360,metadata,recordings,recordingslong,screens,v4k,vdron,videos}`, `002_finales`, `003_curado`. |
| SSD1 / XSSD / MEDIA501 | No conectados al inspeccionar | Sin comprobar | Roles relatados por el propietario; no se inventaron rutas ni identidades. |

ESSD y MEDIA502 reportan distintos discos físicos y UUID de volumen. Ambos reportan `Removable=false` aunque sean USB externos: filtrar sólo por ese booleano perdería destinos reales. Guardar identidades en configuración local al registrar discos; los UUID personales no necesitan publicarse en este documento. No usar `disk4`/`disk6` como identidad persistente. Esta inspección no diagnostica la causa de desconexiones de ESSD.

Con 33.78 GB libres, una importación de 80 GB no debe empezar hacia ESSD. Puede archivarse en MEDIA si la política permite sólo archivo; si trabajo es obligatorio, queda bloqueada con el espacio requerido visible. No debe ocupar silenciosamente el disco interno.

## Hallazgos priorizados

| Prioridad | Evidencia | Consecuencia / decisión |
| --- | --- | --- |
| P0 | `move_preserving_metadata`, `move_exact_preserving_metadata` y sidecars comparan tamaño y luego borran. `hprj/bmp/gyro/aae` se eliminan directamente. | Un respaldo futuro debe copiar, verificar SHA-256 leyendo el destino y confirmar un recibo antes de considerar limpieza. El importador actual sigue siendo legado destructivo. |
| P0 | Sony retira clips, sidecars y `SONY_SUPPORT_PATHS`, incluidos índices/soporte. | Hipótesis fuerte para el aviso de cámara; no prueba física de causalidad. Conservar tarjeta/estructura durante respaldo; después liberar con perfil de limpieza validado o acción en cámara. No dejar la SD llena como resultado normal ni improvisar la edición de sus índices. |
| P0 | El recorrido genérico separaba MOV/WAV de Stalker/proyectos Clipper. | Corregido en esta entrega: rechazo previo de paquetes, subdirectorios dentro de ellos y paquetes anidados; también links/archivos especiales. No se implementó su transferencia todavía. |
| P1 | `city` es el tercer argumento posicional de save. | `save all mx clipper-tui-intro-01` etiqueta la ciudad con ese texto. Separar proyecto, ciudad, país y perfil en el nuevo planner; no reinterpretar históricos a ciegas. |
| P1 | `detect_sd_source_name` prioriza cualquier estructura Sony; sólo detecta Sony/Insta360. `save all` sólo mira nombres de volumen del mapa. | Una estructura residual puede clasificar toda la SD mal. Clasificar por archivo y unidad de captura; nombre de tarjeta es evidencia débil. |
| P1 | Ruta con `mavic2` decide `vdron`; resolución decide `v4k`; DJI no tiene detector por modelo. | DJI 360, dron y acción no pueden distinguirse por marca, extensión MP4 o resolución. Modelo/metadatos y confirmación por unidad deben preceder al ruteo. No se asumió qué modelo DJI 360 tienes. |
| P1 | Las raíces sólo se validan con `is_dir`; no hay verificación de montaje, UUID o espacio. | Registrar volúmenes y validar identidad/dispositivo antes de toda escritura. Un directorio residual no prueba montaje. |
| P1 | Audio valida SSD1 al encontrar cada archivo, después de poder procesar otros. | Preflight antes del primer byte. Por defecto H6 usa MEDIA+SSD1; con MEDIA y sin SSD1 se puede confirmar excepción sólo archivo y registrar trabajo pendiente/omitido. |
| P1 | No hay journal, lock de operación, checksum o idempotencia; colisiones reciben sufijos. | Una repetición puede producir duplicados. El fallo deja destinos parciales sin resultado estructurado. |
| P1 | Clipper importa Stalker por referencia; cambiar raíz o digest produce error. Relink genérico no actualiza toda `capture_sessions`. | No mover sesiones detrás del editor. Resolver por identidad y relink completo con ACK del consumidor; colas/snapshots también son dependencias. |
| P1 | Stalker escribe durante captura y finaliza derivados al detener. | `screen.mov` existente o tamaño estable no significa sesión terminada. Requerir `completed` y revalidación antes/después de copiar. |
| P2 | Downloads es destino real configurado de Stalker y default de salidas/proyectos de Clipper. | Mover nuevas capturas a biblioteca durable configurable. Descargas queda para revisión/export temporal elegido, sin barridos destructivos globales. |
| P2 | `subidos` expresa publicación por convención. | Importar esa convención como sugerencia. Conservar estado por entrega/variante y evidencia de publicación; no marcar todo un proyecto publicado porque se subió un short. |
| P2 | Atlas conserva JPG+DNG, exports de vuelo y estado humano separado de derivados reconstruibles. | No romper parejas ni renombrar inputs que Atlas ya referencia. Atlas sigue siendo autoridad geoespacial; el catálogo media registra ubicaciones y dependencias. |

## Cambios hechos en esta entrega

- Retiro del renderer/editor y experimento IA internos, sus ejemplos y pruebas exclusivas. `nu9ve clip` devuelve error con el enlace al editor vigente. No migra JSON de rangos a proyectos modernos.
- `nu9ve inspect DIRECTORIO`: JSON de observaciones por archivo, bundles como unidad, estado Stalker y evidencia local Sony/Insta360/DJI. No ejecuta FFprobe ni calcula hashes, no determina destinos y no copia/borra. DJI sigue marcado para revisión. Es una base de descubrimiento, no un plan seguro de importación.
- Protección previa de `save` genérico para Stalker/Clipper y links/entradas especiales. Es un control previo, no aislamiento contra cambios concurrentes o una transacción de `save all`.
- Contrato propuesto, diseño de arquitectura/UX y protocolo de validación. No hay watcher, catálogo SQLite, copia verificada, relink automático ni app Swift implementados todavía.

## Recomendación

Construir primero un motor de transferencias revisable y reanudable; luego una app macOS pequeña que lo invoque. Conservar rutas históricas y migrar por incorporación al catálogo, evitando reorganizar terabytes para estrenar la UI. La app debe mostrar por separado «disponible para editar», «archivado verificado», «redundancia» y «publicado».

Los detalles están en [arquitectura y almacenamiento](media-architecture.md), [flujos e interfaz](media-experience.md), [contrato entre repos](contracts/README.md), [prueba Sony](sony-validation.md) y [plan incremental](../PLAN.md).
