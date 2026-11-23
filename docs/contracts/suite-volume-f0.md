# Identidad portable y adopción de discos en nüün

F0 usa [identidad de dispositivo](../../vendor/nu9ve-core/contracts/nu9ve-device-v1.md), [identidad de volumen](../../vendor/nu9ve-core/contracts/nu9ve-volume-v1.md) e [índice de datos](../../vendor/nu9ve-core/contracts/nu9ve-data-v1.md). El motor Rust es dueño de estas operaciones. Ajustes → Discos ofrece el registro en la suite, la publicación explícita y «Adoptar este disco…».

- El registro en la suite añade `.nu9ve/volume.json` a la raíz montada y crea el `device.json` común si hace falta. Las pruebas y QA usan `NU9VE_DEVICE_FILE` absoluto. El registro de destinos del catálogo local conserva sus propios ids y prefijos.
- El segundo factor es `VolumeUUID` en macOS y `VolumeSerialNumber` en Windows, comparados dentro de cada sistema. Los avistamientos se guardan en `suite-volumes/` junto al catálogo local. Una identidad duplicada o un serial contradictorio detienen el uso del disco hasta que se revise. Una copia confirmada conserva el id anterior en `previous_volume_id` y necesita publicar su propio índice.
- Publicar el índice requiere confirmar que los nombres, las rutas relativas de las copias de ese disco y sus proyectos pueden viajar con él. `.nu9ve/apps/nuun/index.json` describe los archivos presentes que ya figuran en el catálogo, con hash y estado de la última verificación cuando están disponibles. La raíz lleva el id portable. Los proyectos usan su id local estable hasta que se les asigne una clave común de suite.
- El índice referencia los recibos de `.ncreator/receipts/` y, si el archivo ya lo lleva, el snapshot más reciente de `.ncreator/catalog-backups/` con su recibo. Esto permite adoptar los metadatos ya escritos en MEDIA sin migrar originales ni duplicar el snapshot en la carpeta nueva.
- Adoptar valida el marcador y el índice de hasta 8 MiB, muestra su fecha y comprueba el SHA-256 del snapshot cuando está declarado. Las raíces por id sólo se resuelven con segundo factor en estado `ok`. La adopción funciona con un catálogo local vacío y no escribe en la máquina ni en el disco. Registrar un avistamiento local es una operación separada.

## RPC de una sola ejecución

Usar `nuun-engine rpc` con el sobre `{id, method, params}`.

| Método | Parámetros |
| --- | --- |
| `suite_device` | Opcional `name` para nombrar o confirmar el dispositivo; `peek: true` lo lee sin crear `device.json` (Ajustes → Discos) |
| `suite_register_volume` | `root` absoluta del volumen, `label`, `role` (`archive`, `work`, `other`) |
| `suite_observe_volume` | `root` absoluta. Registra avistamientos compatibles en el estado local |
| `suite_resolve_volume` | `root`, `volume_id` y `serial` observados, `decision` (`same` o `copy`) |
| `suite_publish_index` | `root` de un destino local registrado, `include_personal: true` |
| `suite_adopt_volume` | `root`, opcional `include_index: true` para obtener también el documento |

[Estado y validación física](../STATUS.md#identidad-portable-f0--2026-11-22).
