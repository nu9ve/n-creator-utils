# nüün · Stalker y espacio · 2026-10-26

Build local **`affd140113f9`**. Artefacto `dist/nuun-stalker-space-20261026.app`, instalado en `~/Applications/nuun.app`; anterior preservado en `~/Library/Application Support/n-creator-utils/app-backups/fee0e0a5.app`. El primer intento de instalación se detuvo porque había una instancia abierta; el usuario la cerró y autorizó instalar. La app no se abrió ni se probó visualmente. Runtime externo `/usr/bin/python3`, backend incluido, firma ad-hoc; no release de producción.

## Implementado

- Registro Stalker `stalker-library/1`: biblioteca actual/anteriores, carpeta manual adicional, errores visibles y eventos por biblioteca/registro, sin polling en reposo.
- Bandeja: proyecto/país por sesión, sugerencias `manifest.project` v1 confirmables y tags locales por UUID; el productor conserva sus manifests. Selección por lote permite proyectos y países distintos por unidad; el layout queda congelado.
- Espacio: auditoría con SHA-256 actual (incluidos archivos con recibos), plan por carpeta de raws o bundle Stalker original ya archivado, cobertura por contenido para nombres históricos. Conserva editor projects, renders y auxiliares ajenos.
- Retiro explícito con ID/digest del plan registrado, identidad de discos/origen, hashes de archivo/origen, comprobación de dependencias antes y durante el retiro, journal por archivo y reanudación. Nunca elimina material no verificado ni toma un informe de cobertura como autorización de borrado.
- Clipper: consulta CLI `library deps`, reenlace explícito por CLI con ACK validado/persistido; prefiere la copia completa verificada en ESSD para retirar un original local, con MEDIA como alternativa. Bloquea colas/snapshots que fijan rutas, offline, errores/versiones desconocidas y proyectos antiguos no adoptados detectados en ubicaciones conocidas. No escribe JSON de Clipper.
- Snapshot SQLite consistente y verificado en MEDIA: todas las tablas, perfiles, tags, decisiones humanas e integración/ACKs. Botón explícito y respaldo antes/después del retiro y después de ingreso por `apply`.

[Flujo de usuario y recuperación del catálogo](../operations.md#bandeja-stalker-proyectos-y-liberar-espacio).

## Evidencia ejecutada

- Python: **134/134** con `python3 -m unittest discover -s tests -v`, 37.848 s en el pase final.
- 36 pruebas nuevas en `tests/test_media_release.py`: descubrimiento actual/anterior/manual, sugerencias/tags/rename, país por unidad, original activo/sin recibo, copia preferida de trabajo, metadata SQLite, hashes, corrupción, desconexión, mismo disco, material nuevo, cancelación, crash tras unlink y tras podar carpetas vacías, reanudación, vínculos/colas antiguas, ACK incompleto, refresco de planes y frontera CLI/RPC. Subprocesos Clipper son fixtures; no se ejecutó el Clipper personal.
- Swift: **7/7**, compilación de todas las vistas, build release y `codesign --verify --strict` del artefacto.
- `python3 -m compileall -q media manager nu9ve.py scripts`, `git diff --check`, 115 enlaces locales en documentos editados, incluido este cierre.
- Ningún inventario/copia/borrado/rehash sobre medios personales. No otros repos modificados, no commits/staging global ni mensajes a otras sesiones.

## Aceptación y límites pendientes

1. Primera apertura y aceptación visual las hace el usuario: bibliotecas encontradas, tags de sesiones, tamaño/scroll de Bandeja/Espacio, plan y confirmaciones.
2. Configurar ejecutable de Clipper en Ajustes → Capturas si no está en PATH/ubicaciones habituales. Un error bloquea liberar espacio; no bloquea el respaldo.
3. Primera operación pequeña real de Stalker → MEDIA/ESSD → reenlace → retirar original, con edición/exportaciones detenidas. No hay lease compartido: no editar ni importar las mismas fuentes simultáneamente al retiro.
4. Proyectos viejos que siguen usando rutas locales sin adoptar la biblioteca requieren resolver la adopción/relink en Clipper; no hay migración automática desde nüün. Proyectos/consumidores fuera de registros conocidos (p. ej. Final Cut) requieren revisión humana. No se promete descubrirlos todos.
5. La auditoría ESSD/MEDIA completa real no se ejecutó. El ingreso de raws antiguos no acepta bibliotecas de editor; el nuevo retiro conserva esos proyectos y renders. Faltan backup general de proyectos editoriales y validación física por filesystem/cámara.
6. Los snapshots no tienen rotación ni restauración integral desde UI; la guía permite restaurar SQLite a un estado nuevo. Preferencias visuales de macOS, informes derivados y JSON privados de otras apps no forman parte de ese backup.
7. El caso de fotos EOS_DIGITAL / kasakarla-electroritual sigue sin operación real ni aceptación física de limpieza Canon. Ingest/limpieza de SD mantiene el motor previo.
8. Persisten gates de [STATUS](../STATUS.md) / [release](../release.md), incluyendo endurecimiento de autorización del plan de ingreso (el plan de retiro nuevo sí se registra), owner/lease, ubicación multijob y distribución.
