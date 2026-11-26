# Discos de trabajo, conversaciones y sugerencias · diseño

Actualizado: 2026-11-24. En la M4, Atlas procesa en SUBSSD. En la M1, nove explora con EDICION y MEDIA8/502 sirve de archivo. nüün copia entre discos y respalda conversaciones por dispositivo. Se apoya en el core: [volumen](../../vendor/nu9ve-core/contracts/nu9ve-volume-v1.md) §7–§8, [datos](../../vendor/nu9ve-core/contracts/nu9ve-data-v1.md) §2.1 y §6, [agente](../../vendor/nu9ve-core/contracts/nu9ve-agent-v1.md), [intercambio](../../vendor/nu9ve-core/contracts/nu9ve-exchange-v1.md) y [varias máquinas](../../vendor/nu9ve-core/docs/suite-devices.md) §6.4–§6.5; y en nove A70–A76. Estado y evidencia en [STATUS](../STATUS.md#discos-de-trabajo-y-conversaciones--2026-11-23).

## Principios

- **nüün copia; las demás apps registran.** nove nunca copia entre discos (A70, A76). nüün copia verificado y deja un recibo que la otra app usa sin volver a hashear.
- **Cada app escribe sólo su carpeta.** En un disco de trabajo, nüün escribe sus datos y `.nu9ve/apps/nuun/`; lee las carpetas de las demás. En MEDIA (`archive`) escribe sólo nüün.
- **Plan revisado antes de escribir; nada destructivo.** Ninguno de estos flujos borra. Liberar espacio sigue exigiendo recibos verificados de la app dueña.
- **Lo personal sale sólo por decisión explícita,** guardada, con fecha y visible en Ajustes. Respaldar en un disco es esa decisión para ese disco (data §6), y se toma por colección.
- **Sugerir, no insistir.** Una sugerencia aparece sólo si hay algo nuevo, una vez por evento (conectar un disco), y se puede callar para siempre por disco y flujo.

## 1. Copiar trabajo entre discos de trabajo

**Hecho en el motor** ([contrato](../contracts/suite-work-copy-v1.md)): `volume_apps`, `work_copy_plan`, `work_copy_apply`, `work_copies`.

- Origen y destino por su marcador (`ok`/`unverified`), los dos `work`/`other` y en discos físicos distintos.
- La copia conserva rutas y fechas, se verifica leyendo del dispositivo y se renombra sin reemplazar; lo idéntico se adopta y lo distinto bloquea el plan.
- nüün lee los índices de las demás apps del origen para decir qué datasets y proyectos toca, y guarda copias exactas de los archivos de esas apps **en su propia carpeta** del destino (`.nu9ve/apps/nuun/copies/<id>/source/<app>/…`).
- **Sí, la copia lleva un recibo de nüün** (`nuun-work-copy/1`, en los dos discos): disco, carpetas y hash de cada archivo de origen y destino, el manifiesto y los índices de origen. Con él nove registra el destino sin hashear (contrato, «Qué hace con esto otra app»).
- **Ventana:** Discos → «Copiar al otro disco…» y la sugerencia en Inicio. La hoja lista los **proyectos de nüün** del disco de origen (de su índice publicado) además de las carpetas de las demás apps. **«Actualizar lo que cambió»** reemplaza lo distinto guardando la versión anterior en la carpeta de nüün del destino, y la sugerencia `work_copy_changed` propone la vuelta cuando lo copiado cambió en un disco. **Falta:** un proyecto de Clipper con sus dependencias (`paths` desde `clipper.inspect`) y resumir lo copiado en el índice de nüün del destino. El orden para el recorrido M1 → M4 → M1 está en el [PLAN](../../PLAN.md#recorrido-de-misiones-entre-la-m1-y-la-m4-pedido-del-usuario-2026-11-23).

### Permiso, registro local y marcador portable

- **Permiso de macOS** habilita a nüün para leer y escribir en el volumen según sus permisos.
- **Añadir SSD para editar…** guarda el destino y prefijo en el catálogo de esta Mac. Con eso aparece como destino en Ajustes y en la vista Discos.
- **Registrar en la suite** escribe `.nu9ve/volume.json` con identidad y rol portables. La copia entre discos lee ese marcador en ambas raíces montadas y funciona aunque el EDICION sólo esté registrado en el catálogo de la M1. **Adoptar este disco…** consulta el índice publicado, en sólo lectura para el volumen. La adopción no añade un destino al catálogo local.
- En la M4, para copiar con el EDICION que ya trae su marcador, abrir **Discos → Copiar al otro disco…** y usar **Elegir disco…** para seleccionar ambas raíces en `/Volumes`. Si también se quiere usar el EDICION como destino de edición o verlo en el mapa de Discos, añadirlo como SSD para editar en esa Mac.
- El índice del EDICION consultado el 24 nov ofrece `subaereo` por la ruta `001_proyectos/atlas-subaereo/misiones/2026/11`. La build de esa primera copia requería elegir `_missions/` y `_nuun/` manualmente. En el código actualizado, la hoja los incorpora al elegir la carpeta de proyecto indexada, según la receta. Los resultados bajo `atlas-derived/` se eligen por separado.

## 2. Respaldo de conversaciones por dispositivo

**Hecho en el motor** ([contrato y formatos para nove](../contracts/suite-conversations-v1.md)): `conversations_settings`, `conversations_status`, `conversations_backup_plan`, `conversations_backup_apply`, `conversations_run` y `conversations_export_working`; `living_plan` acepta `include`.

Colecciones conocidas, cada una con sólo su **historial** (la clase `history` de `storage_collections`). Nunca credenciales, ajustes, cachés ni registros:

| Colección | Raíz | Entra |
| --- | --- | --- |
| `claude-code` | `~/.claude` | `projects`, `history.jsonl`, `todos`, `plans`, `transcripts`, `file-history`, `sessions` |
| `codex` | `~/.codex` | `sessions`, `archived_sessions`, `history.jsonl`, `session_index.jsonl*`, `attachments`, `generated_images`, `visualizations`, `dictation-history`, `transcription-history.jsonl` |
| `claude-desktop` | `~/Library/Application Support/Claude` | `local-agent-mode-sessions`, `claude-code-sessions` |

- **Respaldo** = una colección viva (`living_plan`/`living_apply`, versiones conservadas) en un disco `archive` elegido, en `<base>/<colección>/<device_id>/…`, con `<base>` = `004_datos/conversaciones` por omisión. Cada máquina escribe su carpeta; un MEDIA reúne todas sin mezclarlas.
- **Recibo** por respaldo en `<base>/<colección>/<device_id>/_nuun/receipts/<plan_id>.json` (`nuun-conversations-backup/1`): colección, dispositivo, archivos copiados, versionados y sin cambios, bytes, y el hash del manifiesto de la colección (`_nuun/manifest.json`, que ya escribe la colección viva).
- **Copia de trabajo (opcional):** después del respaldo, un espejo verificado en un disco `work`/`other` o una carpeta de análisis, con la misma forma `<base>/<colección>/<device_id>/…`, para que nove la lea en sitio. Un archivo que creció (las sesiones se agregan) se reemplaza sólo después de verificar la versión nueva; las versiones viejas quedan en el archivo.
- **Decisión explícita y visible.** Ajustes → Conversaciones guarda, por colección, «respaldar en <disco>» con su fecha y, aparte, «dejar copia de trabajo en <destino>». Sin esa decisión no se planea nada. `conversations_status` dice qué está configurado y cuándo se decidió.
- **Cuándo:** «A pedido» siempre; «Sugerir cada N días» por omisión (N = 7); «Automático» sólo si la persona lo elige: con la app abierta y el disco conectado, respalda sin preguntar cuando toca.
- **Para nove:** la carpeta `<device_id>` es la identidad de la máquina (UUID de `device.json`); `_nuun/manifest.json` lista cada archivo con tamaño y SHA-256; el recibo dice colección y nombre del dispositivo. El proyecto es la clave conocida `conversaciones`.

### Reunir los historiales de M4 y M1 en EDICION

La build instalada exige un destino de archivo accesible desde la Mac que contiene los historiales. Con MEDIA en la M4, elegir en Ajustes → Conversaciones las colecciones y SUBSSD como copia de trabajo, y ejecutar «Respaldar ahora». Con MEDIA y ambos SSD conectados, se puede elegir EDICION directamente como copia de trabajo. Si se pasa por SUBSSD, copiar a EDICION sólo `004_datos/conversaciones/<colección>/<id-M4>/` por cada colección autorizada. La M1 puede escribir su propia copia al mismo EDICION cuando respalde sus historiales a MEDIA. Las carpetas por dispositivo preservan ambas procedencias. Está pendiente validar el recorrido con discos reales y reunir ambos historiales en nove desde ese árbol.

Sin MEDIA, **Ajustes → Conversaciones → Exportar historial a un SSD de trabajo** permite elegir las colecciones y confirmar el SSD. La exportación reutiliza el formato `nuun-conversations-working/1`, con carpeta por dispositivo y manifiesto SHA-256. Puede repetirse a pedido. Los archivos que cambian durante la lectura quedan para la próxima exportación. Para llevar la copia al EDICION, seleccionar las carpetas de ese dispositivo con «Copiar al otro disco…» y revisar «Actualizar lo que cambió» si ya hay una versión anterior. El flujo de respaldo a MEDIA conserva sus versiones de archivo.

En la M1, el motor de nove permite resumir Claude Code y Codex apuntando al EDICION, sin abrir las carpetas activas. En este ejemplo `ID_M4` es la carpeta de dispositivo del export. La pantalla de Conversaciones aún requiere validar la incorporación de ambos dispositivos. El lector de Claude Desktop de nove todavía tiene el formato pendiente.

```sh
history_root=/Volumes/EDICION/004_datos/conversaciones
device_id=ID_M4
/Applications/Nove.app/Contents/Helpers/nove-engine conversations-summary \
  --claude-projects "$history_root/claude-code/$device_id/projects" \
  --codex-home "$history_root/codex/$device_id" \
  --utc-offset-minutes -420 --counts-only
```

## 3. Sugerencias al conectar

**Hecho en el motor:** `suite_suggestions` y `suggestions_dismiss` (`{key, dismissed}`), por `rpc`. En la ventana, Inicio → «Discos conectados», con «Ahora no» y «No volver a preguntar».

`suite_suggestions` (lectura) mira los discos montados y la configuración y devuelve sólo lo que tiene algo que hacer:

| Flujo | Cuándo aparece |
| --- | --- |
| `work_copy` | Dos discos de trabajo conectados y una raíz de otra app en uno (su `volume_path`) que no existe en el otro ni está en un recibo de copia hacia él |
| `work_copy_changed` | Los dos discos comparten un recibo de nüün sin carpeta de destino adicional. Bajo una ruta de ese recibo aparecen archivos nuevos o cambian su tamaño o fecha desde la última copia. Detecta candidatos y abre la revisión, donde se verifican los archivos. |
| `conversations_backup` | El disco de respaldo elegido está conectado y una colección decidida no se respalda desde hace N días (o nunca) |
| `conversations_working` | Fuera del modo «A pedido», una carpeta de trabajo configurada quedó detrás del último respaldo. Actualizar vuelve a ejecutar el respaldo en MEDIA. |
| `conversations_update` | Al entrar en Inicio, al volver a su ventana o al conectar un SSD de trabajo. Un destino elegido en Ajustes o con una copia previa de esta Mac tiene archivos nuevos o modificados respecto a su manifiesto. La tarjeta confirma el destino y ejecuta el export directo, incluso sin MEDIA. Reaperturas dentro de diez segundos reutilizan el resultado. |

- Cada sugerencia tiene una clave estable (`flujo:volume_id[:…]`). «No volver a preguntar» la guarda en el catálogo local para ese disco y flujo (`suggestions_dismiss`). «Ahora no» la calla hasta el próximo evento: la app recuerda, en memoria, qué sugirió por montaje.
- Las SD siguen con su ventana de ingreso de siempre; esto no la repite.

### Pasada de UX pendiente

1. **Una sola alta para el caso común.** Al elegir «Usar como disco de trabajo», integrar el alta local y la confirmación para escribir el marcador portable. Si ya existe un marcador compatible, reutilizarlo y ofrecer «Usar en esta Mac» junto al disco detectado. Mantener disponible la consulta del índice en sólo lectura. Mostrar en un solo lugar qué está montado, qué se puede consultar y qué está configurado como destino.
2. **Sugerencia por proyecto y dirección.** Para la primera actualización, usar evidencia de que se trata del mismo proyecto y una relación explícita entre proyecto, par de discos y dirección, o un historial de copias. Para Atlas/Subaéreo, permitir elegir las entradas y las carpetas de resultados al establecer esa relación. Sugerir sólo cuando haya material pendiente, indicando proyecto, origen, destino, rutas y motivo. Discos con proyectos distintos pueden seguir sin sugerencias. El tipo de proyecto orienta qué carpetas revisar. Si hubo cambios en ambos discos, mostrar ambas direcciones y pedir revisión antes de reemplazar.
3. **Sugerencias de conversaciones por destino y colección.** Inicio ofrece actualizar desde la Mac al SSD elegido o ya usado. Sigue pendiente sugerir actualizaciones entre dos SSD a partir de una elección de destino y cambios de los historiales de cada colección y dispositivo.
4. **Transferencias largas.** La hoja «Copiar al otro disco…» usa la vista compartida `ActionProgressView` mientras copia y verifica, y al terminar vuelve al recibo. Queda validar en ventana el cambio de fase con una copia de varios GB.

## 4. Otras Macs, por el agente

- «Trabajar aquí» ya usa `lease_take`/`lease_release` (F1). Después de copiar un proyecto a un disco, la sugerencia en la otra Mac es «Trabajar aquí» en ese proyecto.
- Las conversaciones de otras máquinas no viajan por la carpeta de intercambio: van en el disco (respaldo o copia de trabajo). Por el agente sólo se ve su forma (`where`), como manda data §6.
- No se toca el agente instalado, el llavero ni los LaunchAgents: todo se prueba con `NU9VE_AGENT_DIR` y carpetas de QA.

## Decisiones del usuario

**Decididas (el usuario, 2026-11-23),** tal como se propusieron:

1. **Carpeta de las conversaciones en MEDIA:** `004_datos/conversaciones/<colección>/<device_id>/`, al lado de `004_datos/clipper/transcripts`.
2. **Qué entra:** sólo historial (tabla de arriba). Las bases vivas de Codex (`thread_history*.sqlite`, 1,6 GiB en la M1) quedan **fuera** porque Codex las escribe mientras corre. Si un día entran, será por un snapshot (`VACUUM INTO`).
3. **Cuándo:** «Sugerir cada 7 días» por omisión; «Automático» sólo si la persona lo elige.
4. **Copia de trabajo:** la persona elige disco o carpeta en Ajustes; no hay destino por omisión. Una vez elegida, va con cada respaldo.
5. **Copias de MEDIA a un disco de trabajo** no entran en `work_copy`: son «Entregar trabajo».
6. **Recibo también en el origen** de una copia entre discos: sí.
