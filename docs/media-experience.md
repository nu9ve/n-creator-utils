# Flujos y rediseño de la experiencia de media

Documento de experiencia objetivo para macOS. Ya existe una primera app de ingreso/asignación/plan/jobs/biblioteca; no todos los controles aquí propuestos están terminados. Estado real: [STATUS](STATUS.md). Próximas superficies: [dashboard/launcher](features/dashboard-launcher.md). La [arquitectura](media-architecture.md) define intención y límites.

## Barra de menú y acciones rápidas

En reposo: icono discreto. Al conectar SD, primero comprobar destinos registrados montados. Si no hay MEDIA ni SSD de trabajo (ESSD/XSSD/SSD1), no mostrar popup ni notificación, ni escanear profundamente: se asume una SD para consultar otro contenido. Con un destino disponible, una notificación abre el panel mínimo de ingreso; disponibilidad del panel no implica que estén todos los destinos requeridos. Un evento no abre la ventana grande ni inicia un movimiento destructivo. Agrupar eventos duplicados; recordar «ignorar esta tarjeta hasta que vuelva a conectarse» y mostrar cola si entran varias. Si conectas después un destino y la SD sigue presente, reevaluar una vez.

Panel sugerido:

```text
Nueva tarjeta · 12 unidades · 18.4 GB              [ejemplo]
Sony ILCE-6400: 8   DJI por identificar: 4

Perfil       [erikiado64 ▾]
Proyecto     [clipper-tui-intro-01           ] [Usar último]
País / ciudad [MX ▾] [Hermosillo ▾]
Trabajo      [ESSD · e64_raw ▾]   Archivo [MEDIA502 ▾]

[Revisar y separar proyectos]       [Ver plan]
```

Las cifras son ilustrativas. En operación se muestran datos medidos, evidencia y pendientes. País y ciudad tienen defaults independientes; el proyecto anterior se ofrece pero no se aplica silenciosamente a una tarjeta nueva. Perfil puede recordar destino y tipo editorial sin asumir que Sony siempre es e64.

`Ver plan` muestra filas/unidades con origen, clasificación, nombres propuestos, destinos, bytes, auxiliares, saltos por duplicado y advertencias. Encabezado: «SD se limpia después de verificar», réplicas requeridas, espacio disponible y pendiente. Conflicto de cámara o destino obligatorio ausente deshabilita ejecutar el plan directo. Si está MEDIA y falta el SSD de trabajo, ofrecer la excepción confirmada descrita abajo. Acción principal: «Respaldar y liberar SD». «Sólo copiar» es una opción explícita que conserva la fuente; es diferente de «Sólo respaldar en MEDIA», que omite trabajo y puede liberar SD tras verificar archivo. La limpieza queda incluida en el plan; perfiles validados no necesitan una segunda confirmación al terminar. Si Sony requiere liberación en cámara, advertirlo antes de empezar.

Con defaults válidos y toda la tarjeta del mismo proyecto: proyecto → revisar resumen → copiar. Se puede habilitar inicio tras completar asignaciones por una preferencia explícita; el default conserva el paso de revisión del plan. «Guardar todo» significa todas las unidades seleccionadas, no todos los archivos conectados a la computadora.

## MEDIA disponible sin ESSD o SSD1

Mostrar «Falta SSD1/ESSD. Puedes respaldar en MEDIA; los archivos no quedarán en tu disco de trabajo». El usuario elige conectar el SSD o **confirmar «Sólo respaldar en MEDIA»**. Esta excepción requiere una confirmación adicional explícita por operación; no se hereda silenciosamente al siguiente ingreso.

En esa confirmación: «Dejar pendiente para copiar a mi SSD después» como elección inicial, o «Omitir copia de trabajo». El resumen final muestra por separado «MEDIA verificado», «SD liberada» y «Trabajo pendiente/omitido». La SD puede limpiarse tras cumplir la política de sólo archivo confirmada, sin fingir que se hicieron dos copias.

Al conectar el SSD: «N proyectos respaldados pendientes de preparar para trabajo». Elegir proyectos/unidades y «Copiar desde MEDIA» o «Omitir». Si MEDIA falta, indicar que hay que conectarlo; no exigir la SD original. Omitidos no vuelven a aparecer como pendientes en cada conexión, aunque pueden recuperarse manualmente. Copia diferida verificada sin duplicar lo ya entregado.

Si está sólo ESSD/SSD1 y falta MEDIA, puede aparecer el panel porque existe un destino gestionado, pero no ofrecer la excepción «sólo archivo» ni limpiar SD automáticamente: falta el archivo requerido. Este caso no se confunde con la ausencia total de destinos, que mantiene la app silenciosa.

## Tarjetas con varios proyectos

Vista de asignación con thumbnails bajo demanda, duración, fecha, cámara y selección múltiple; orden por captura con opción de ordenar manualmente. Seleccionar 4 clips → asignar proyecto A → desaparecen del filtro «pendientes». Seleccionar 2 → proyecto B, etc. Deshacer/asignados permite corregir errores.

País/ciudad/perfil/proyecto son campos distintos. Se pueden cambiar por lote; no concatenar títulos de proyectos para resolver la mezcla. Material sin proyecto puede guardarse conscientemente en Inbox, pero no interpretarse como asignación completada por accidente. Si una toma pertenece a dos proyectos, conservar relaciones a ambos y una sola identidad de contenido.

JPG+DNG, sidecars y segmentos de una captura se asignan juntos. Una `.stalker` no se fragmenta para archivar aunque se use en varios proyectos o sólo parte de su duración. Pertenencia editorial no obliga a duplicar todos sus bytes para cada proyecto.

La detección presenta «Sony por XML: modelo…», «DJI sin modelo confirmado» o «sin clasificar». Una carpeta Sony vacía no reclasifica los DJI. Primera cámara nueva permite alias y categoría dron/360/acción/cámara; guardar regla con evidencia y alcance. Un override del usuario se registra con su razón; cambios de modelo vuelven a revisión, sin una regla global «DJI = dron».

## Progreso, desconexiones y recuperación

Mostrar fases y destinos por separado: copiando trabajo, verificando trabajo, copiando archivo, verificando archivo, registrando, limpiando SD y lista para grabar. «Trabajo listo» puede aparecer antes de «Archivo verificado», permitiendo abrir Clipper con una ubicación estable. No marcar finalizado sólo por alcanzar 100% de bytes escritos ni llamar «SD lista» al estado de respaldo verificado con limpieza pendiente.

Si se desconecta ESSD: «Pausado · falta ESSD registrado». Conservar SD y journal; al reconectar confirmar identidad automáticamente si coincide y revalidar el intento. Otro disco con el mismo nombre no continúa el job. Cancelar conserva fuente y deja staging identificable/reanudable; la limpieza del staging es una acción separada que muestra espacio recuperable.

## Stalker como biblioteca de originales

Elegir una carpeta de capturas estable, visible y fuera de Downloads para nuevas sesiones. Seguir capturando mientras se archivan sesiones anteriores: el watcher sólo considera sesiones `completed`; no toca `recording`, `preparing`, `finalizing`, `failed` o `interrupted`. Estas últimas van a revisión/recuperación en Stalker.

Al llegar una sesión terminada: relacionar perfil/proyecto reciente como sugerencia, conservar `sessionID` y bundle entero. Sin MEDIA puede seguir pendiente de archivo y ser usada desde su biblioteca local. Al conectar MEDIA ofrecer «Archivar N sesiones terminadas». Copiar sólo completadas; no bloquear grabaciones nuevas por hacer hashes de otras sesiones. Limitar IO para evitar competir con captura.

Nueva UI de Clipper propuesta: «Agregar desde biblioteca» con filtros proyecto/fecha/perfil y estado de disco. Elegir unidad local o de trabajo disponible; conservar pegado directo de rutas como fallback. Incluir cámaras y Stalker en orden explícito. Sugerir proyecto/autogroup según asociación y cercanía temporal; la correlación y los presets siguen bajo autoridad del editor.

Mover después de haber importado requiere relink por identidad y respuesta de Clipper; hasta eso conservar la ubicación usada. Si una cola antigua retiene ruta previa, debe fijarla o recibir relink/invalidación controlada. No basta con encontrar otra copia de `screen.mov` por nombre.

## Del raw al publicado

Conservar tus convenciones y añadir seguimiento:

| Etapa | Acción y comportamiento sugerido |
| --- | --- |
| Ingreso | Raw en MEDIA y selección de trabajo en `001_proyectos/e64_raw/<proyecto>`; otro perfil elige su propia raíz. |
| Clipper | Agregar unidad/selección desde biblioteca; preset creativo propuesto por perfil, confirmado en el editor. Mantener orden de selección al crear queue. |
| Revisión | Export a Downloads puede seguir como elección rápida. Distinguir versión candidata y elegida, vinculadas a entrega/proyecto. |
| Final Cut | Material elegido a `001_proyectos/e64/<proyecto>`. Registrar dependencias; no asumir que borrar raw es seguro por existir un render. |
| Final | `002_finales/e64` conserva master fecha+título y variantes `_vertical_N`. Un manifest de entrega une variantes, thumbnails, subtítulos y copy elegidos. |
| Publicación | Confirmación por variante/canal. Mover a `subidos` produce sugerencia de estado para compatibilidad, revisable antes de automatizar. |
| Archivo final | Al conectar MEDIA, plan de entregas publicadas pendientes → `002_finales/<perfil>` según configuración histórica, copia verificada idempotente. No pasar finales por `save` de cámara. |
| Liberar trabajo | Revisar cobertura de archivo y dependencias abiertas de Clipper/colas/Final Cut/Atlas. Después proponer retiro de copias de trabajo, conservando catálogo offline y recuperación. |

Entrega completa y master no equivalen a backup del proyecto editable. Final Cut puede depender de bibliotecas, plugins y medios externos; definir primero qué nivel de reedición deseas preservar y reflejarlo como cobertura incompleta hasta resolverlo.

## Ventana grande bajo demanda

Navegación: **Pendientes**, **Biblioteca**, **Proyectos**, **Discos**, **Entregas**, **Actividad**, **Ajustes**. El icono de barra expone pendientes/progreso/último resultado y «Abrir biblioteca»; no mantiene thumbnails ni una webview pesada ocultas.

Biblioteca muestra miniaturas paginadas, proyecto/perfil, captura y ubicación: trabajo online, archivado offline o pendiente. Acciones «Restaurar a…», «Preparar selección para Clipper», «Revisar copias» generan planes. Discos muestra libre/reservado, colecciones y última conexión; distingue datos desconocidos de cero bytes.

La primera entrega nativa sólo necesita panel de ingreso, revisión de clasificación y jobs. Catálogo avanzado y exploración pueden llegar después sin impedir respaldar bien desde la barra.

## Otros brazos

- **Downloads:** proveedor → staging en raíz elegida → resultado estructurado → registrar procedencia/asset → organizar. Evitar depender de texto libre de `scdl`; no borrar una descarga parcial sin mostrarla. No descargar al cwd accidentalmente desde la app.
- **Stems:** un job derivado con hash del original, herramienta/modelo/parámetros, outputs y errores. No renombrar/mover la única fuente de un proyecto existente; versión de trabajo y cache con cuota.
- **Hardware/MIDI:** herramientas optativas fuera del proceso idle. Inventariar usos de scripts y S3 antes de retirar o importar prototipos.
- **Atlas:** selector de colección por vuelo/proyecto; preservar pares y exports; diferenciar raw permanente, working set y derivados seleccionados. Procesamiento no se dispara por el simple evento de montar un disco.

## Validación de UX y recursos

Probar primero con copias: tarjeta de un proyecto; tarjeta mixta; DJI ambiguo; SSD1 ausente; SSD sin espacio; desconexión durante copy/verify/cleanup; archivos nuevos antes de limpiar; Stalker grabando y otra sesión terminada; reintento idéntico; entrega con sólo algunos shorts publicados. Fallos previos a verificación conservan origen; éxito de ingreso con limpieza deja SD libre y usable. Un fallo durante limpieza conserva los respaldos y muestra qué miembros faltan por retirar.

Medir app en idle, UI abierta, captura concurrente y job activo con CPU/RSS/wakeups/IO; fijar presupuesto tras baseline del equipo. Aceptación de idle: sin workers de FFprobe/hash ni escaneo recursivo periódico, observers activos y sin crecimiento sostenido de memoria. Windows/Linux quedan diseñados, sin validación de producto todavía.
