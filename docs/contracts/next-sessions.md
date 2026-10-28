# Próximas sesiones: Stalker y luego Clipper

Actualización 2026-10-28: Stalker ya implementó el productor coordinado y nüün
ya despacha recuperación pending. Ver [protocolo vigente](stalker-retitle-v1.md#recuperación-pending-adoptada-por-nüün--2026-10-28)
y evidencia en STATUS. Las instrucciones previas de implementar esa capability
son históricas; falta adopción operativa/física con builds actualizadas.

## Prioridad vigente · 2026-10-28

1. **Stalker:** adoptar `stalker-coordinated-retitle/1` mediante las tres APIs
   exactas capabilities/status/apply, herencia de locks y journal recuperable.
   Leer primero el [handoff compacto y especificación](stalker-retitle-v1.md),
   [schema](stalker-retitle.schema.json) y fixtures enlazadas. El coordinador
   de nüün ya está implementado; no duplicar deps/ACK/relink. Mantener inmutable
   el manifest archivado; `archived:false` no autoriza. No usar capturas reales.
2. **Clipper:** consumir campos aditivos de sources descritos en
   [productor](media-library-v1.md#productor-aditivo-de-biblioteca--28-octubre-2026),
   conservar compatibilidad con v1 anterior y rechazo de revisión nueva sin
   adopción explícita. No asumir que implementó el handoff portable. Validar
   con las mismas fixtures, sin modificar manifests de Stalker.

Los prompts de 2026-10-25 y hallazgos de abajo son históricos; no sustituyen
esta prioridad ni autorizan instalación, captura, copia o liberación física.

Escritos el 2026-10-25. Son dos sesiones separadas que se corren en este orden:

1. **Stalker** fija primero cómo nacen las sesiones: nombre, biblioteca, metadata y señal de archivo.
2. **Clipper** implementa después contra lo que ya exista aquí y en Stalker, y lo deja fluido para importar y editar.

Las dos pueden y deben actualizar el contrato en este repo según las [reglas](README.md#cómo-actualizan-este-contrato-otras-sesiones).

## Seguimiento nuevo para la próxima sesión de Stalker · 2026-10-27

La sesión de implementación descrita abajo ya ocurrió. Antes de continuar con Clipper, revisa estos dos hallazgos del lote reciente de dos sesiones y deja evidencia en tu propio repo; actualiza sólo `docs/contracts/*.md` de n-creator-utils siguiendo la fila Stalker de adopción y las reglas de este repo.

1. **Nombre de carpeta.** Las capturas se ven con nombres `Stalker_yyyy-MM-dd_HH-mm-ss.stalker` aunque se les asignó `terra-6-1` al preparar el lote. Reproduce con fixtures y sigue el orden de guardado de proyecto, finalización, escritura durable de `completed` y rename. Aclara si asignar el proyecto en nüün cambia metadata del productor (no debería: el tag del archivador vive en su catálogo) o si el nombre genérico es esperado cuando Stalker no recibió proyecto durante la captura. Confirma cuándo se aplica la gramática final, el comportamiento con colisión/error/interrupción y que `sessionID` + digest siguen siendo la identidad aunque el nombre quede genérico. No edites sesiones reales ni reescribas bundles completados para investigarlo.
2. **Tamaño del micrófono.** En las mismas sesiones, compara el archivo de pantalla y la pista de micrófono: bytes, duración, bitrate efectivo, contenedor, codec, sample rate, canales y profundidad de bits, además de los ajustes de grabación que eligió el código en runtime. Determina si la pista es PCM/no comprimida u otra codificación y explica cuánto tamaño sería esperable para esa duración. No reduzcas calidad ni cambies codec hasta identificar la causa y proponer una comparación de captura controlada.

Entrega: hallazgos reproducibles con fixtures o mediciones no destructivas, explicación de si cada comportamiento es correcto o requiere fix, límites, y cualquier petición para el productor en «Pendientes para n-creator-utils». No marques el contrato adoptado por una suposición ni alteres el manifiesto de una sesión archivada.

## 1. Stalker (`~/code/ai/stalker`)

Estado: ejecutada el 2026-10-25 (Stalker `ecb18f1` y `d88eaf9`, main). Resultado en la [fila de adopción](README.md#estado-de-adopción-por-app) y en [naming v1](media-naming-v1.md#adopción-stalker-implementada-2026-10-25).

```text
Eres la sesión de Stalker. Objetivo: terminar la implementación para que Stalker quede dentro del contrato del
ecosistema de media (n-creator-utils archiva y prepara; Stalker captura; Clipper edita) y coordinado con los
flujos ya implementados en n-creator-utils.

Lee primero, en /Users/nu9ve/code/nu9ve/n-creator-utils:
- docs/contracts/README.md (tabla de adopción, "Interfaz local estable", "Cómo actualizan este contrato")
- docs/contracts/media-naming-v1.md (gramática, identidad, "Estado de archivo para productores", "Adopción: Stalker")
- docs/contracts/media-library-v1.md, docs/CONTEXT.md y docs/media-experience.md ("Stalker como biblioteca de originales")
Y en tu repo docs/clipper-contract.md (stalker-clipper/1 sigue siendo tu autoridad de captura: no cambies reloj,
pistas, audio ni internos del bundle).

Estado real hoy: Stalker nombra Stalker_yyyy-MM-dd_HH-mm-ss.stalker, graba en ~/Downloads (disco interno con
~5.7 GB libres) y escribe sessionID en mayúsculas. n-creator-utils ya archivó 3 sesiones del proyecto
"debloateando mi tele" a MEDIA502 + ESSD, renombrando sólo la carpeta de las copias, p. ej.
26OCT24-195850_STALKER_03-48_MX-DEBLOATEANDO-MI-TELE[].stalker, y `nu9ve media captures ~/Downloads` las
reporta archived=true.

Implementa:
1. Biblioteca durable y configurable (default fuera de ~/Downloads, p. ej. ~/Movies/Stalker), con espacio libre
   visible y aviso antes de grabar con poco espacio. Las sesiones existentes se quedan donde están. Decide cómo
   se entera n-creator-utils de esa ruta (hoy su app tiene un campo "Carpeta Stalker a observar") y registra la
   decisión en el contrato. Si requiere algo del productor, agrégalo a "Pendientes para n-creator-utils".
2. Proyecto y país opcionales por sesión, guardados como metadata opcional versionada del manifest. El último
   proyecto se ofrece, pero no se aplica en silencio a una sesión nueva. Sin proyecto, todo funciona como hoy.
3. Naming en origen: con proyecto, al finalizar (ya con la duración conocida) y ANTES de escribir
   status=completed, renombra la carpeta de forma atómica a
   YYMONDD-HHMMSS_STALKER_MM-SS_PAIS-PROYECTO[].stalker (inicio = createdAt en hora local; duración =
   manifest.duration redondeada a segundos; slug en mayúsculas; "-2[]" si hay colisión). Un bundle completed
   nunca se renombra ni se reescribe después. Sin proyecto se conserva el nombre actual. Prueba de contrato:
   createdAt 2026-10-25T02:58:50Z, duration 227618267500/1e9 s, MX, "debloateando mi tele",
   TZ America/Hermosillo → 26OCT24-195850_STALKER_03-48_MX-DEBLOATEANDO-MI-TELE[].stalker
4. Señal de finalización robusta: completed se escribe al final y de forma atómica, y una interrupción nunca
   deja un bundle "completed" a medias. Documenta qué pasa si alguien edita un bundle ya completed (el digest del
   manifest cambia y el archivador lo verá como no archivado).
5. "Liberar sesiones archivadas": usa `python3 /Users/nu9ve/code/nu9ve/n-creator-utils/nu9ve.py media captures
   <biblioteca>` (JSON {ok,data:{sessions:[{path,session_id,status,archived}]}}). Sólo las archived=true, por
   selección explícita del usuario, van a la Papelera (recuperable). Si el comando falla o no existe, no ofrezcas
   liberar. Nunca borres por un evento.
6. Actualiza docs/clipper-contract.md: el nombre de la carpeta no es identidad; se valida por manifest.json
   (producer, sessionID); campos nuevos opcionales del manifest.

Reglas: pruebas sólo con carpetas temporales y un catálogo aislado (NU9VE_MEDIA_HOME o --state-dir a un temp);
no toques las sesiones reales de ~/Downloads ni el catálogo del usuario. Sin dependencias nuevas.

Contrato: al terminar, en n-creator-utils edita sólo docs/contracts/*.md. Actualiza la fila "Stalker" de la tabla
de adopción (estado, fecha, commit, límites). Registra cualquier decisión que afecte a las 3 apps en el contrato
correspondiente, y lo que necesites del productor en "Pendientes para n-creator-utils". No edites código, pruebas,
STATUS ni PLAN de ese repo.

Entrega: pruebas en verde (incluida la de contrato), commit en tu repo si te lo pido, y un resumen con lo
adoptado, las decisiones registradas y los pasos para probarlo yo a mano: grabar una sesión con proyecto, ver el
nombre, archivarla desde n-creator-utils y liberarla.
```

## 2. Clipper (`~/code/ai/clipper`), después de Stalker

Estado: ejecutada el 2026-10-25 (Clipper, working tree sobre `e6981b1`, sin commit). Resultado en la [fila de adopción](README.md#estado-de-adopción-por-app), en [naming v1](media-naming-v1.md#adopción-clipper-implementada-2026-10-25) y en «Pendientes para n-creator-utils».

```text
Eres la sesión de Clipper. Objetivo: terminar la implementación para que Clipper quede dentro del contrato del
ecosistema de media y coordinado con lo ya implementado en n-creator-utils y en Stalker, con features fluidos:
importar un proyecto de la biblioteca en un paso y editarlo.

Lee primero, en /Users/nu9ve/code/nu9ve/n-creator-utils:
- docs/contracts/README.md: tabla de adopción. Revisa qué dejó Stalker y sus decisiones, la "Interfaz local
  estable" y "Cómo actualizan este contrato".
- docs/contracts/media-naming-v1.md ("Identidad", "Adopción: Clipper") y docs/contracts/media-library-v1.md
- docs/CONTEXT.md y docs/media-experience.md ("Stalker como biblioteca de originales", "Del raw al publicado")
Y en ~/code/ai/stalker docs/clipper-contract.md (actualizado por la sesión de Stalker).

Estado real hoy:
- `python3 /Users/nu9ve/code/nu9ve/n-creator-utils/nu9ve.py media projects` lista "debloateando-mi-tele".
- `... media sources --project debloateando-mi-tele` devuelve en una llamada sus 10 fuentes en orden de captura:
  7 clips Sony y 3 carpetas .stalker en /Volumes/ESSD/001_proyectos/e64_raw/debloateando-mi-tele/, con
  asset_id, revision, kind, session_id, path, sha256, captured_at, duration_seconds y alternatives (la copia de
  MEDIA502). Los nombres llevan "[]".
- Tres clips se solapan en el tiempo con una sesión Stalker (19:57/19:58, 20:44/20:44, 20:54/20:56).
- clipper_tui.stalker.expand_inputs ya ordena bien esa carpeta.

Implementa:
1. Quick import "Importar proyecto de la biblioteca": elegir con `media projects`, traer `media sources
   --project` y crear o actualizar el proyecto Clipper con las fuentes en ese orden. Stalker por import de sesión,
   cámara como items. Guarda por fuente asset_id, revision, session_id y sha256/bytes. El profile llega como
   sugerencia. Reimportar es idempotente: agrega sólo assets nuevos y no duplica. Se conservan el pegado de rutas
   y la importación de carpeta, que reconoce bundles por manifest.json y no por el patrón Stalker_*.
2. Autogroup cámara + pantalla. Lee "Agrupar cámara y pantalla" en media-naming-v1.md: la relación es N:M
   (p. ej. C0003 20:41:10-20:43:27 y C0004 20:44:01-20:48:48 alrededor de la sesión 20:44:16-20:52:00). Usa
   solape o cercanía con tolerancia configurable sobre captured_at/duration_seconds y reloj Stalker, con relojes
   de cámara y Mac que pueden diferir. Es un hint que el usuario confirma o deshace; nunca auto-sync silencioso.
3. Robustez de rutas con "[]" y cualquier nombre .stalker en todo el flujo: sync, transcripción, overlays, render y
   export (escape de filtros ffmpeg subtitles=/movie=, listas concat, glob(), shell). Pruebas con fixtures
   temporales que usen esos nombres.
4. Borrado manual, offline y relink: el usuario borra en Finder, sin avisar, clips inútiles o un proyecto entero
   al terminar, y ESSD puede no estar montado. `media sources` ya lo reporta (removed_from_work, offline,
   warnings): muéstralo sin error ni re-import automático. Ofrece relink a la copia de MEDIA sólo si el usuario lo
   pide, validando asset_id/session_id/sha256. Nunca cambies en silencio a otra revisión.
5. Espacio: el disco interno tiene ~5.7 GB libres. Para proyectos importados de la biblioteca, el proyecto y los
   renders van por default a /Volumes/ESSD/001_proyectos/e64/<proyecto>/ (configurable), con aviso de espacio
   antes de un render grande.
6. Nunca escribas dentro de bundles ni de media de la biblioteca, y no renombres archivos de media.

Reglas: pruebas con catálogo aislado (`--state-dir` a un temp, volúmenes con `media volumes register ...
--allow-local`) o con el JSON simulado; nunca uses ESSD, MEDIA502 ni el catálogo real en pruebas automáticas.
Leer el catálogo real sólo para la prueba manual final está bien.

Contrato: al terminar, en n-creator-utils edita sólo docs/contracts/*.md. Actualiza la fila "Clipper" de la tabla
de adopción (estado, fecha, commit, límites). Registra decisiones que afecten a las 3 apps, y lo que necesites
del productor (campos, comandos, bugs) en "Pendientes para n-creator-utils". No edites su código, pruebas,
STATUS ni PLAN.

Entrega: pruebas en verde, commit en tu repo si te lo pido, y un resumen con lo adoptado y los pasos exactos
para probarlo yo a mano: abrir Clipper, importar "debloateando-mi-tele" desde la biblioteca, revisar orden y
agrupación, editar y renderizar.
```

## Después

De vuelta en n-creator-utils:
- revisar la tabla de adopción y los «Pendientes para n-creator-utils» que dejen;
- implementar esos pendientes y reflejar en STATUS lo que reporten;
- quitar `relayout` del flujo diario cuando Stalker ya nombre en origen (queda para material viejo).
