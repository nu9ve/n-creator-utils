# Copias entre discos de trabajo · `nuun-work-copy/1`

Estado: **propuesta**, implementada en el motor Rust (2026-11-23), probada con discos de mentira. Responde al flujo 1 de [discos y conversaciones](../features/suite-disks-flows.md): pasar lo procesado de un disco de trabajo a otro (XSSD → ESSD) verificado, con fechas y con un recibo que otra app (nove) puede usar para registrar el destino **sin volver a calcular hashes**. Se apoya en [`nu9ve-volume-v1`](../../vendor/nu9ve-core/contracts/nu9ve-volume-v1.md) §5, §7 y §8 y en la [transferencia verificada](../media-architecture.md) de nüün.

## Reglas

- **Discos por su marcador.** Origen y destino se eligen por su raíz montada y se reconocen por `.nu9ve/volume.json` en estado `ok` o `unverified` (volumen §5). `suspect`, `duplicate` o sin marcador: no se copia. Los dos deben tener rol `work` u `other` y ser discos físicos distintos. Un `archive` no entra: archivar es el ingreso, y sacar de MEDIA es «Entregar trabajo» (`deliver_work`).
- **Plan revisado.** `work_copy_plan` lista cada archivo, su sello (tamaño, fecha, inode) y su destino, comprueba espacio y conflictos y guarda el plan con su digest en el catálogo local. Nada se escribe en los discos al planear.
- **Mismas rutas.** Cada archivo conserva su ruta dentro del disco; `into` (opcional) antepone una carpeta. Así la raíz de otra app (`volume_path`) es la misma en los dos discos.
- **Copia verificada.** Cada archivo se escribe como `.nombre.ncpartial`, se lee de vuelta del dispositivo, se compara su SHA-256 y se renombra sin reemplazar. Conserva la fecha de modificación del original. Un archivo idéntico que ya está se adopta; uno distinto bloquea el plan, salvo con `update: true` (abajo). Una copia interrumpida se reanuda con la misma llamada.
- **Actualizar (`update: true`).** Para volver con lo procesado: un archivo que ya está en el destino con otro contenido entra al plan con `existing: "different"` y el SHA-256 de esa versión. Al copiar, si el destino sigue siendo esa versión, se mueve (renombrar en el mismo disco, sin reemplazar) a `.nu9ve/apps/nuun/copies/<id>/previous/<ruta>` y después se copia y verifica la nueva. Si el destino cambió desde la revisión, el archivo no se toca y la copia se detiene. Interrumpida entre mover y copiar, la versión anterior ya está a salvo y la reanudación copia la nueva. Nada se borra.
- **Proyectos de nüün.** El índice que nüün publica en un disco (`.nu9ve/apps/nuun/index.json`, un dataset por proyecto y carpeta) dice qué carpetas tiene cada proyecto. La ventana arma `paths` con esas carpetas, así otra Mac copia un proyecto sin el catálogo de la que lo ingresó. Por eso conviene «Publicar índice» del disco antes de llevarlo a la otra Mac. El índice de nüün también viaja en `source_indexes` y sus proyectos van en `context.titles`.
- **Datos de misiones para Atlas.** Si una carpeta de proyecto coincide con `work_folder` de la receta efectiva, `volume_apps` señala `work_copy_companions`: `units_map` y los archivos `history.json` y `changes.ndjson` de `work_mirror`, bajo el mismo prefijo del disco. Al seleccionar esa carpeta indexada, la ventana añade los tres archivos al plan revisado. Un archivo ausente detiene el plan normal por `source_missing`. Una ruta explícita por RPC conserva el alcance que le indicó ese cliente.
- **Nada se borra**, en ningún disco. Liberar el origen sigue exigiendo los recibos de su app dueña (volumen §0).
- **Las carpetas de otras apps se leen, nunca se escriben.** nüün lee `.nu9ve/apps/*/index.json` del origen (con tope de 8 MiB, validado como `nu9ve-data-v1`; uno inválido se informa y no se usa) para decir qué datasets y proyectos toca la copia. De cada app cuyos datasets toca la copia, guarda una copia exacta y verificada de sus archivos de primer nivel (hasta 16 MiB cada uno: `index.json`, `inplace-v1.json`…) **dentro de la carpeta de nüün** del destino.
- **Lo personal:** en el plan y el recibo, de un dataset `personal` (o sin clase) va sólo su forma (id, ruta, modalidad, tamaño, estado, proyectos), nunca su título (data §6). Las copias exactas de los índices de otras apps viajan tal cual porque ya estaban en el disco por decisión de la persona; no salen de sus discos.

## Recibo

Al terminar, el mismo documento se publica, sin reemplazar y releído, en `.nu9ve/apps/nuun/copies/<id>.json` **en los dos discos** (el destino dice de dónde vino; el origen, adónde fue), y queda en el catálogo local (`work-copy-plan:<id>`). JSON canónico ([`nu9ve-json-v1`](../../vendor/nu9ve-core/contracts/nu9ve-json-v1.md)).

```json
{"contract":"nuun-work-copy/1","id":"<uuid>","plan_digest":"<sha256>",
 "copied_by":{"device_id":"<del dispositivo, o null>","app":"nuun"},
 "created_at":"…Z","completed_at":"…Z",
 "source":{"volume_id":"<XSSD>","label":"XSSD","paths":["atlas-derived/potholes"]},
 "destination":{"volume_id":"<ESSD>","label":"ESSD","into":""},
 "file_count":2,"bytes":34,"copied":2,"adopted":0,"replaced":0,
 "manifest":{"algorithm":"sha256","of":"manifest","value":"<sha256>","verified_at":"…Z"},
 "files":[{"source_path":"atlas-derived/potholes/day/a.geojson","path":"atlas-derived/potholes/day/a.geojson",
           "size":28,"mtime_ns":1700000000000000000,"sha256":"<sha256>"}],
 "source_indexes":[{"app":"nove","file":"inplace-v1.json","bytes":34,"sha256":"<sha256>",
                    "copy":".nu9ve/apps/nuun/copies/<id>/source/nove/inplace-v1.json"}],
 "context":{"projects":["subaereo"],"apps":[{"app":"nove","generated_at":"…Z","datasets":[…]}]}}
```

- `files[].path` es la ruta en el destino y `source_path` la del origen, las dos relativas a su disco; `sha256` es el de la lectura de verificación del destino; `mtime_ns` es la fecha conservada.
- Un archivo actualizado lleva `previous: {sha256, path}`: el hash de la versión que reemplazó y dónde quedó en el destino. `replaced` los cuenta.
- `manifest.value` es el SHA-256 de las líneas `ruta TAB sha256\n` ordenadas por ruta de destino: la misma regla que el índice de nüün (`of: "manifest"`, volumen §8).
- `source_indexes[].copy` es una ruta del destino, dentro de la carpeta de nüün, con los bytes exactos del archivo de esa app en el origen al planear.

## Qué hace con esto otra app (nove)

Para registrar en ESSD lo que nove indexó en XSSD sin hashear:

1. Leer `.nu9ve/apps/nuun/copies/*.json` del disco (con tope), con `contract` `nuun-work-copy/1` y `destination.volume_id` igual al marcador del disco.
2. Tomar su propio archivo de `source_indexes` (`app: "nove"`), comprobar que su SHA-256 es el declarado, y leerlo como si fuera el de `source.volume_id`.
3. Para cada dataset suyo cuyos archivos estén en `files` (por `source_path` relativo al disco), usar `path` como ruta en este disco y `sha256` como hash ya verificado. Un archivo que no está en `files` no se copió.
4. El estado sigue siendo el vivo: un archivo cambiado desde `completed_at` (otro tamaño o fecha) es `changed`, nunca `ok`.

El recibo no autoriza borrar nada en el origen.

## RPC

Por `nuun-engine rpc` (sobre `{id, method, params}`):

| Método | Parámetros | Qué hace |
| --- | --- | --- |
| `volume_apps` | `root` | Marcador, estado y, por app, su índice (`ok`, `absent`, `invalid` con su motivo), sus datasets resueltos a rutas del disco y sus archivos; y los recibos de copias del disco. Sólo lee |
| `work_copy_plan` | `source`, `destination` (raíces), `paths` (1 a 64 rutas relativas, sin `.nu9ve`, `.ncreator` ni `_nuun`, sin solaparse), opcional `into`, `update`, `progress_id` | El plan revisado, con `digest`, conteos, `adopt`, `replace`, `context` y `carried` |
| `work_copy_apply` | `plan_id`, `digest` | Copia, verifica, publica el recibo y devuelve el plan con `state: completed` y `receipt` |
| `work_copies` | — | Los planes y recibos del catálogo local |

Errores propios: `unmarked`, `suspect`, `duplicate`, `invalid_role`, `same_volume`, `same_physical_disk`, `destination_conflict`, `insufficient_space`, `source_missing`, `unsafe_file`, `volume_replaced` (un disco cambió desde la revisión), `plan_changed`.

## Sugerencia de vuelta

`suite_suggestions` propone `work_copy_changed` (clave `work_copy_changed:<origen>:<destino>`, con `update: true`) cuando dos discos de trabajo conectados tienen recibos entre ellos y, bajo las carpetas copiadas, uno tiene archivos nuevos o con otro tamaño o fecha que la última copia en cualquier sentido. Compara sólo tamaño y fecha, con dos segundos de margen porque HFS+ guarda segundos y exFAT 10 ms, sin hashear ni necesitar el índice de otra app. No cuenta las copias con `into`.

## Pendiente

- Un proyecto de Clipper con sus dependencias: armar `paths` desde `clipper.inspect` (deps) en lugar de una carpeta.
- Que el índice de nüün del destino resuma también lo copiado como un dataset con su `hash` de manifiesto.
- Prueba física con XSSD y ESSD reales ([release](../release.md)).
