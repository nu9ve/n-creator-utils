# Mapa de almacenamiento

Subtab Discos de Biblioteca: [navegación y flujos](product-flows.md). **Fase 1 implementada**, con evidencia en [STATUS](../STATUS.md); las acciones de fases posteriores están marcadas abajo.

Herramientas como DaisyDisk o GrandPerspective dicen cuánto ocupa algo. nüün puede decir además **si es seguro retirarlo**, porque sabe qué tiene copia verificada, de qué proyecto es y quién depende de ello.

## Preguntas que responde

1. ¿Qué está llenando este disco?
2. ¿Qué existe sólo aquí?
3. ¿Qué puedo liberar sin riesgo, y cuánto?
4. ¿Esto ya lo tengo en otro disco, aunque esté desconectado?

## Mediciones que la justifican

Tomadas el 2026-10-30 en sólo lectura. MEDIA502 estaba desconectado.

En GiB, como `df -h`; la app muestra GB decimales (449 GiB son 482 GB).

| Dato | Valor |
| --- | --- |
| ESSD | 445 GiB usados de 465 GiB, 80,061 archivos |
| Recorrido de metadatos de ESSD con `du` | 10.4 s |
| Conocido por el catálogo en ESSD | 19.1 GiB (cerca del 4 %) |
| `001_proyectos` / `0000_acomodar` / `002_finales` | 234.6 / 141.1 / 43.0 GiB |
| Interno | 888 GiB usados de 926 GiB, 12 GiB disponibles |

**Conclusión de diseño:** recorrer metadatos es barato; lo caro es hashear. La auditoría actual (`media coverage`) no guarda hashes entre corridas y, en el peor caso, toma horas. Por eso el índice se separa de la verificación: el mapa nunca lee contenido por sí solo. Falta medir el recorrido en el HDD MEDIA502.

## Índice

Swift y la CLI piden el recorrido al servicio JSON. [Python](../../media/storage_index.py) y [Rust en el sandbox de macOS](../../engine/crates/nuun-media/src/storage/write.rs) comparten el esquema del índice y el bloqueo `storage.lock`.

Base **separada** del catálogo (`storage-index.sqlite3`): es derivada y reconstruible, no bloquea al escritor del catálogo y no infla sus instantáneas. Las decisiones humanas, como reglas de carpeta o adopciones, sí viven en el catálogo.

Guarda ejecuciones, un índice publicado por disco, archivos, carpetas con sus totales, clasificación y hashes. Un escaneo escribe aparte y sólo reemplaza el índice anterior al terminar: cancelarlo o perder el disco deja el anterior intacto.

Para que el índice nunca sea lo que llena un disco casi lleno:

- En discos registrados guarda cada archivo.
- En carpetas elegidas guarda archivos desde 10 MB y carpetas desde 50 MB. Lo demás queda sumado en su carpeta padre.
- Devuelve el espacio de lo reemplazado y se detiene si quedan menos de 512 MB libres.

### Cómo evita recalcular

- Un hash se reutiliza mientras `(volume_id, path, size, mtime_ns)` no cambie.
- Los recibos del catálogo siembran `hashes` con `origin=receipt`, sin leer el archivo.
- Una comparación sólo hashea candidatos del mismo tamaño, los más grandes primero, y guarda el resultado. Interrumpirla no pierde lo ya calculado.
- Un escaneo vuelve a leer los metadatos y conserva los hashes de lo que no cambió. Detectar cambios con `FSEvents` entre escaneos queda para cuando haga falta; ExFAT debe comprobarse.

### Un índice no autoriza a borrar

| Evidencia | Permite |
| --- | --- |
| Tamaño igual | Mostrar «probable» |
| Hash en caché o recibo | Mostrar «verificado el <fecha>» |
| Índice de un disco desconectado | Responder «según el índice del <fecha>, está en MEDIA» |
| **Rehash actual de origen y destino** | **Liberar**, como ya hace `plan_release` |

Una huella rápida (tamaño + inicio/fin, como `q1` en Kyū) puede ordenar candidatos, nunca probar identidad. La corrupción silenciosa que no cambia tamaño ni mtime la cubre el rehash periódico pendiente en la [política de réplicas](replication-policy.md).

### Cuándo se escanea

- **Manual** con «Actualizar», con cancelación y progreso real.
- **Opcional al conectar** un disco registrado, en Ajustes. Desactivado por defecto.
- **Nunca** en reposo ni sobre SDs. Se mantiene sin sondeo periódico.
- Los discos desconectados muestran su último índice con fecha, nunca como si fueran datos actuales.

## Clasificación

Dos ejes independientes, intercambiables en la vista.

**Seguridad**

| Clase | Significado | Acción principal |
| --- | --- | --- |
| En respaldo | Mismo SHA-256 en el respaldo, por comparación o recibo. Las dependencias se revisan al liberar | Revisar liberación |
| Probable | Mismo tamaño en el respaldo, sin hashes en ambos lados | Comprobar |
| Sólo aquí | Sin copia en el índice del respaldo | Respaldar |
| Regenerable | Declarado así por su app o por la persona | Revisar retiro |
| Sin revisar | Aún sin comparar | Comprobar |

Una carpeta sólo toma una clase cuando al menos el 90 % de su tamaño la tiene. Si no, aparece como **Mixto**, con su reparto a escala: media carpeta sin copia nunca debe verse segura.

Dos carpetas en un mismo disco físico nunca cuentan como réplica. Esto se pregunta mientras ambos discos están conectados, porque los números de disco cambian entre conexiones.

**Contenido:** video, RAW, foto, audio, captura Stalker, proyecto de edición, regenerable y otros. Se usa la extensión, y los paquetes (`.stalker`, `.fcpbundle`, proyecto Clipper) se tratan como una sola unidad, sin ofrecer acciones sobre su interior.

Un nombre como `Caches` o `node_modules` sólo produce una sugerencia; nada se retira por su nombre. La vista por proyecto llega con la fase 2.

**Esta Mac** siempre aparece en la lista. Analiza el volumen de datos del disco interno (`/System/Volumes/Data`): todo menos el sistema sellado. Se trata como una carpeta elegida, sin poder olvidarla.

**Carpetas elegidas:** **Añadir carpeta…** permite ver otra carpeta u otro disco. Viven fuera de la tabla de discos, así que nunca pueden elegirse como archivo o trabajo ni activan el aviso de tarjetas. Se rechazan la raíz, las carpetas del sistema, las tarjetas y lo que se solape con otro disco.

El uso real del sistema de archivos menos lo recorrido se muestra como **«Sin contar»**: sistema, instantáneas, purgable o carpetas fuera de alcance. Mejor un hueco explícito que un total que no cuadra.

## Vista

```text
┌ Discos ───────────────────────────────────────────────────────────┐
│ ESSD 445/465 GB · hoy 10:42   MEDIA502 · desconectado, índice 28 oct │
│ [██████ proyectos ████ acomodar ██ finales ░ libre]               │
│                                                                   │
│ 141 GB sin clasificar · 19 GB en catálogo · 0 GB liberables aún   │
│                                                                   │
│ Ver por: [Seguridad] Contenido Proyecto    Actualizar · Comprobar │
│ ESSD › 001_proyectos                                              │
│ ┌ mapa ──────────────────┐  Carpeta          Tamaño  Estado       │
│ │ visuales  │ atlas-der. │  visuales         83.7 GB Sin revisar  │
│ │           ├─────┬──────│  atlas-derived    34.1 GB Sin revisar  │
│ │           │ 0cl │ ...  │  erikiado         24.0 GB Sin revisar  │
│ └────────────────────────┘  [ panel de la carpeta y sus acciones ]│
└───────────────────────────────────────────────────────────────────┘
```

- **Respuestas primero:** arriba, la barra del disco y su leyenda con bytes por clase. Los ejemplos de esta maqueta no son resultados de una comparación.
- **Lista como vista principal, treemap como compañía.** La lista ordena, se lee con VoiceOver y permite atajos. El treemap da la forma de un vistazo. Se prefiere a un sunburst porque aprovecha mejor el área y deja espacio para etiquetas.
- **Dibujo:** SwiftUI `Canvas`, sin dependencias, con el resto agrupado en «otros» por nivel. Datos paginados por carpeta, sin enviar 80,000 filas al modelo.
- **El color no es la única señal:** cada clase lleva icono y texto. Respeta Reduce Motion, y el modo oscuro usa la paleta existente.
- **Vacío y error:** sin índice, se muestra `df` y el botón para escanear; con índice antiguo, la fecha; con error de permisos, qué carpeta y cómo otorgar acceso.

## Acciones

Ninguna borra directamente. **No hay botón «Eliminar».**

| Acción | Resultado | Disponible |
| --- | --- | --- |
| Mostrar en Finder | Abre la ruta | Fase 1 |
| Comprobar | Hash incremental contra archivo; no modifica nada | Fase 1 |
| Revisar liberación | `plan_release` existente, con rehash actual | Fase 1, sólo en discos de trabajo |
| Respaldar | Plan de sólo archivo; el planner aún rechaza proyectos de edición | Fase 1, sólo en discos de trabajo |
| Adoptar carpeta | Asocia a un proyecto sin mover nada | Fase 3 |
| Respaldar carpeta viva o proyecto de edición | Plan incremental hacia MEDIA | Fase 3 |
| Marcar regenerable o ignorar | Regla guardada en el catálogo | Fase 3 |
| Duplicados | Grupos con mismo SHA-256 y plan de cuál conservar | Posterior |

El retiro de regenerables pasa por un plan explícito. Mandar a la Papelera en un disco externo no libera espacio hasta vaciarla, por lo que no es un sustituto.

## Por acomodar

Un cajón como `0000_acomodar` se revisa por partes. Abre la carpeta y pulsa **Comprobar**:

| Lo que ves | Significa | Qué hacer hoy |
| --- | --- | --- |
| En respaldo | Ya lo tienes, aunque tenga otro nombre | **Revisar liberación…**, que vuelve a verificar antes de retirar |
| Sólo aquí | No está en el respaldo | Decidir. Respaldar con **Respaldar…**, o dejarlo |
| Probable | Falta leer el contenido | **Comprobar** |

Las carpetas de `0000_acomodar` quedan fuera del prefijo de trabajo; si la liberación las rechaza, la fase 3 añade las salidas «sirve» (asignar proyecto y respaldar) y «ya fue» (descartar con confirmación).

## Interfaz del servicio

Con la misma envoltura JSON para CLI y app. Las lecturas van por el lector persistente; escaneo y comparación corren en su propio proceso y nunca comparten cancelación con una transferencia. Un solo análisis a la vez por catálogo.

| Método | Tipo | Función |
| --- | --- | --- |
| `storage_overview` | Lectura | Discos, uso y clases, con fecha del índice |
| `storage_tree` | Lectura | Carpetas y archivos más grandes de una carpeta, paginados |
| `storage_progress` | Lectura | Análisis en curso |
| `storage_scan` | Índice | Escaneo cancelable; sólo metadatos |
| `storage_compare` | Índice | Hash incremental contra el respaldo; no modifica nada |
| `storage_add_folder`, `storage_remove_folder` | Catálogo e índice | Comparten `storage.lock` con escaneo y comparación. Olvidar una carpeta retira sus filas derivadas y su registro; conserva sus archivos. |

MCP expone `media_storage_overview` y `media_storage_tree`, de sólo lectura.

## Pruebas y aceptación

[`tests/test_media_storage.py`](../../tests/test_media_storage.py) usa directorios temporales e identidades de fixture; nunca discos personales. Cubre, entre otros: el escaneo no abre archivos; enlaces sin seguir y hard links contados una vez; cancelar o perder el disco conserva el índice anterior; hashes reutilizados hasta que cambia tamaño o fecha; copias renombradas reconocidas e impostores del mismo tamaño rechazados; un archivo cambiado no se da por bueno; mismo disco físico no cuenta; recibos con nombres descompuestos; índice dañado reconstruido; sin espacio se detiene; y un respaldo alterado después de comparar hace fallar la liberación.

**Aceptación física:** una corrida explícita en ESSD debe cuadrar con `df`, y lo no explicado debe aparecer en «Sin contar». Pendiente medir el HDD MEDIA502, que fija el presupuesto de rendimiento.

## Fuera de alcance

Limpiar el sistema, desinstalar apps, tocar `/System` o `~/Library` como acción de nüün, sincronizar con la nube, servicios en segundo plano.
