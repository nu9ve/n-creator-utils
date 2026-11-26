# Conversaciones por dispositivo · `nuun-conversations-backup/1` y `nuun-conversations-working/1`

Estado: **propuesta**, implementada en el motor Rust (2026-11-23), probada con discos de mentira. Es el flujo 2 de [discos y conversaciones](../features/suite-disks-flows.md#2-respaldo-de-conversaciones-por-dispositivo): las conversaciones de la persona con sus agentes, de cada máquina, en un disco, para que nove las lea juntas bajo la clave conocida `conversaciones` ([`nu9ve-data-v1` §2.1](../../vendor/nu9ve-core/contracts/nu9ve-data-v1.md#21-claves-conocidas)) sin mezclar máquinas.

## Carpetas

```text
<disco>/<base>/<colección>/<device_id>/…     los archivos, con sus rutas dentro de la raíz del agente
                                 └─ _nuun/manifest.json        en el archivo: cada archivo con tamaño y SHA-256
                                 └─ _nuun/receipts/<plan>.json en el archivo: un recibo por respaldo
                                 └─ _nuun/versions/<corrida>/… en el archivo: versiones anteriores
                                 └─ _nuun/working.json         en la copia de trabajo: su manifiesto
```

- `<base>` es `004_datos/conversaciones` por omisión (Ajustes; [decisión 1](../features/suite-disks-flows.md#decisiones-del-usuario)).
- `<colección>`: `claude-code` (raíz `~/.claude`), `codex` (`~/.codex`) o `claude-desktop` (`~/Library/Application Support/Claude`).
- `<device_id>` es el UUID de [`device.json`](../../vendor/nu9ve-core/contracts/nu9ve-device-v1.md) de la máquina que respaldó. Cada máquina escribe sólo la suya: un MEDIA reúne las de todas sin pisarse. Una colección que ya se respaldaba sin carpeta de dispositivo la conserva.
- Debajo, cada archivo conserva su ruta relativa a la raíz del agente (`projects/<repo>/<sesión>.jsonl`, `history.jsonl`, `sessions/2026/…`) y su fecha de modificación.
- Sólo entra el **historial**: las entradas de primer nivel de la tabla del [diseño](../features/suite-disks-flows.md#2-respaldo-de-conversaciones-por-dispositivo). Nunca credenciales (`.credentials.json`, `auth.json`), ajustes, cachés, registros ni las bases SQLite vivas de Codex. Los enlaces simbólicos se saltan y se cuentan.
- `_nuun/` es de nüün: un lector lo excluye de los datos.

## Respaldo (en un disco `archive`)

Es una colección viva de nüün: incremental, verificada por SHA-256 y sin borrar nunca. Un archivo que cambió (una sesión que creció) deja su versión anterior en `_nuun/versions/`. Un archivo que ya no está en el origen se queda.

- `_nuun/manifest.json` (`nu9ve-living-manifest/1`): `files` es un mapa de la ruta relativa a la colección (sin `<base>/<colección>/<device_id>/`) a `{size, sha256, archive_path}`, más `collection_id`, `name`, `source` y `updated_at`.
- `_nuun/receipts/<plan_id>.json` (`nuun-conversations-backup/1`), JSON canónico:

```json
{"contract":"nuun-conversations-backup/1","collection":"claude-code","title":"Claude Code",
 "device":{"device_id":"<uuid>","name":"M1"},"plan_id":"<uuid>","plan_digest":"<sha256>","completed_at":"…Z",
 "archive":{"name":"MEDIA502","folder":"004_datos/conversaciones/claude-code/<uuid>"},
 "include":["projects","history.jsonl","…"],"result":{"copied":2,"adopted":0,"versioned":0,"changed_since_review":0,"versions":[]},
 "bytes":15,"unchanged":{"files":0,"bytes":0},
 "manifest":{"path":"…/_nuun/manifest.json","sha256":"<sha256 del manifiesto al terminar>"}}
```

`device` es `null` si la máquina no tenía `device.json` al escribirlo (el `<device_id>` de la carpeta sigue mandando).

## Copia de trabajo (en un disco `work`/`other` o una carpeta)

Opcional y decidida aparte. Es un espejo del historial con la **misma forma** (`<base>/<colección>/<device_id>/…`), para leer en sitio. Cada archivo se copia a un parcial, se lee de vuelta y se compara su SHA-256, y sólo entonces reemplaza al anterior; conserva la fecha. Lo que ya no está en el origen se queda. Nunca va a un disco `archive`.

`_nuun/working.json` (`nuun-conversations-working/1`):

```json
{"contract":"nuun-conversations-working/1","collection":"claude-code","title":"Claude Code",
 "device":{"device_id":"<uuid>","name":"M1"},"source":"/Users/…/.claude","updated_at":"…Z",
 "include":["projects","…"],"files":{"history.jsonl":{"size":8,"mtime_ns":…,"sha256":"<sha256>"}}}
```

Inicio propone actualizar un SSD portable `work` u `other` si ya tiene un manifiesto de esta colección
y dispositivo, o si la colección y ese destino se eligieron en Ajustes. Compara rutas, tamaños y fechas
del origen con el manifiesto. Las entradas nuevas también conservan `ctime_ns` e `inode` del origen;
las anteriores siguen siendo válidas. El conteo de Inicio usa metadatos del origen y del manifiesto.
La actualización comprueba el destino y verifica por SHA-256 los archivos que copia. Si todo sigue igual,
el export directo conserva también el manifiesto. Los archivos que cambian durante la copia quedan pendientes.

## Lo personal

Todo esto es `personal` ([data §6](../../vendor/nu9ve-core/contracts/nu9ve-data-v1.md#6-privacidad)). Sale de la máquina sólo por la decisión de la persona, por colección (`conversations_settings` con `confirm_personal: true`, que guarda `decided_at`) y, aparte, para la copia de trabajo. Cambiar el disco de respaldo vuelve a pedir la confirmación. `conversations_status` muestra qué está decidido y desde cuándo. Por MCP, como siempre, sólo la forma.

## Para nove

1. Una carpeta `<base>/<colección>/<device_id>/` con UUID válido es el respaldo (o la copia) de esa colección de esa máquina. El nombre de la máquina está en el recibo más reciente o en `working.json` (`device.name`), como dato de la persona.
2. Para registrar sin hashear: en el archivo, `_nuun/manifest.json` (tamaño y SHA-256 verificados al copiar); en la copia de trabajo, `_nuun/working.json` (tamaño, fecha y SHA-256). Un archivo cuyo tamaño o fecha no coincide es `changed`.
3. Un dataset por colección y máquina, en el proyecto `conversaciones`, con `origin_key` `nuun:conversations:<colección>:<device_id>`.

## RPC

| Método | Qué hace |
| --- | --- |
| `conversations_settings` | Sin parámetros, los ajustes. Con `archive` (nombre de un destino `archive`), `base`, `schedule` (`{mode: manual\|suggest\|auto, every_days}`), `collections` (`{id: true\|false}`), `working` (`{root}` o `null`) y `confirm_personal`. Sólo el catálogo |
| `conversations_status` | Ajustes, y por colección: raíz, si está, su historial (entradas, archivos y bytes, sólo `lstat`), `decided_at`, último respaldo y `due` |
| `conversations_backup_plan` | Un plan revisado de colección viva por colección decidida (u opcional `collections`) |
| `conversations_backup_apply` | `plans: [{plan_id, digest}]`: copia, recibo y, si está decidida, copia de trabajo (`working: false` la omite) |
| `conversations_run` | `when: "now"` (el clic de la persona) o `"due"` (sólo con `mode: auto` y lo que toca): planea y aplica |
| `conversations_export_working` | `destination` (raíz de un disco portable `work` u `other`), `collections` (ids elegidos), `confirm_personal: true` y, opcionalmente, `volume_id` para confirmar que sigue siendo el SSD sugerido. Escribe la copia de trabajo y su manifiesto por dispositivo a pedido. Funciona sin un destino `archive`. |

## Estado en la ventana

Ajustes → Conversaciones ofrece el respaldo y el export a pedido. La app abierta llama a `conversations_run {when: "due"}` si corresponde en modo automático para el respaldo. Los archivos que cambian durante el export a pedido se informan para la siguiente ejecución. [Recorrido para reunir historiales en ESSD](../features/suite-disks-flows.md#reunir-los-historiales-de-m4-y-m1-en-essd).
- Las bases SQLite de Codex por snapshot ([decisión 2](../features/suite-disks-flows.md#decisiones-del-usuario)).
