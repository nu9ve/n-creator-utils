# Escritura duradera y acceso a archivos de nu9ve v1

## 1. Escritura atómica

Crea los directorios padres necesarios y un temporal hermano de nombre
`.<nombre>.<sufijo>.tmp`. Abre el temporal con `create_new` y modo
`0600` en Unix. Escribe los bytes completos, sincroniza el archivo,
cierra su descriptor, renombra sobre el destino y sincroniza la carpeta.
Una ruta con sólo el nombre usa la carpeta actual.

Antes del rename, un error conserva el destino y se intenta retirar el
temporal. Si falla la apertura o la sincronización de la carpeta tras
el rename, se devuelve el error e indica que el rename ya ocurrió. Los
bytes nuevos pueden estar visibles aunque su persistencia no esté confirmada.

## 2. Publicar sin reemplazar

Publica un archivo completo y ya sincronizado mediante un enlace duro o un
rename exclusivo. Si ya existe una entrada con el nombre de destino, incluido
un enlace simbólico roto, devuelve un conflicto. El resultado indica si el
original quedó enlazado, se movió o se copió.

Si el origen y el destino están en volúmenes distintos, copia a un temporal
hermano del destino, sincronízalo y publícalo con rename exclusivo. Conserva
el origen para que el llamador pueda retirarlo tras el éxito. Sincroniza la
carpeta de destino y, si un rename quitó un nombre de otra carpeta, también
la de origen. Un error de sincronización posterior puede dejar visible el
destino.

Cuando el sistema no permite enlaces duros, como exFAT, usa el rename. Si
tampoco implementa el rename exclusivo, comprueba la ausencia con
`symlink_metadata` y renombra bajo un candado que respetan todos los
publicadores. La exclusión de ese último paso cubre a los escritores que
comparten el candado.

## 3. Sincronización

- `Full` es el modo predeterminado. Usa `File::sync_all`, que solicita
  `F_FULLFSYNC` en macOS. Cuando el sistema de archivos rechaza esa
  operación por falta de soporte, intenta `fsync(2)`.
- `Plain` se pide de forma explícita. Usa `fsync(2)` en Unix y reintenta
  si se interrumpe con `EINTR`, igual que `os.fsync` de Python.
- Un fallo de sincronización de la carpeta se propaga.

## 4. Candados y sonda

El llamador elige el archivo de candado y cómo abrirlo. El modo exclusivo
impide otros candados sobre el mismo archivo y el compartido permite otros
compartidos. Se intenta adquirir con `flock` sin bloqueo hasta el plazo
medido con reloj monotónico. Plazo cero hace un intento. La contención al
vencer el plazo se distingue de un error de E/S. Cerrar el descriptor libera
el candado. Los participantes mantienen estable la entrada del candado.

La sonda «¿alguien lo tiene?» intenta brevemente un candado exclusivo con
otro descriptor. Cierra el descriptor al salir y no crea un archivo ausente.
Devuelve si había un candado y propaga errores de E/S. El resultado describe
el momento de la sonda.

## 5. Lectura para verificar

Recibe un archivo regular ya abierto y validado. Lee todo su contenido y
permite volver al principio para repetir la verificación. Informa cómo se
hizo la lectura: `Bypassed` si se pidió saltar la caché con éxito,
`Dropped` si se pidió descartar páginas limpias, u `Os` si queda la caché
normal. En macOS usa `F_NOCACHE`. Una verificación necesita el archivo
estable y sus escrituras previas sincronizadas.

## 6. Plataformas

- **Linux.** Ambos modos sincronizan mediante `fsync(2)`. El rename
  exclusivo usa `renameat2(RENAME_NOREPLACE)`. Para verificar se pide
  descartar páginas limpias con `posix_fadvise(POSIX_FADV_DONTNEED)`. Esa
  indicación es consultiva y se registra como `Dropped`. Los candados
  usan `flock`.
- **Windows.** Ambos modos usan `File::sync_all` (`FlushFileBuffers`).
  El temporal hereda las ACL de la carpeta. Para publicar, se usa
  `MoveFileExW` con `MOVEFILE_WRITE_THROUGH`, con la opción de reemplazo
  sólo cuando se pide. Es el mecanismo de persistencia disponible para
  estos renames y sus fallos se propagan. Un flush de carpeta explícito
  usa un handle con `FILE_FLAG_BACKUP_SEMANTICS` y propaga los fallos que
  devuelve Windows. Los candados usan `LockFileEx` sobre archivos abiertos
  para lectura y escritura. Para verificar, `ReOpenFile` reabre el mismo
  archivo con `FILE_FLAG_NO_BUFFERING` y lee en bloques alineados. Si no
  admite esa lectura, vuelve a la lectura normal e informa `Os`.

[Implementación en Rust](../rust/nu9ve-fs/) · [Adopción por app](../docs/adoption.md#nu9ve-fs-v1)
