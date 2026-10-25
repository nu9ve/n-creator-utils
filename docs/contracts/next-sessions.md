# Próximas sesiones: Stalker y luego Clipper

Escritos el 2026-10-25. Son dos sesiones separadas que se corren en este orden:

1. **Stalker** fija primero cómo nacen las sesiones: nombre, biblioteca, metadata y señal de archivo.
2. **Clipper** implementa después contra lo que ya exista aquí y en Stalker, y lo deja fluido para importar y editar.

Las dos pueden y deben actualizar el contrato en este repo según las [reglas](README.md#cómo-actualizan-este-contrato-otras-sesiones).

## 1. Stalker (`~/code/ai/stalker`)

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
2. Agrupación sugerida cámara + pantalla por solape de captured_at/duration_seconds y reloj Stalker, como hint
   que el usuario confirma o deshace. Nunca auto-sync silencioso.
3. Robustez de rutas con "[]" y cualquier nombre .stalker en todo el flujo: sync, transcripción, overlays, render y
   export (escape de filtros ffmpeg subtitles=/movie=, listas concat, glob(), shell). Pruebas con fixtures
   temporales que usen esos nombres.
4. Offline y relink: las copias de trabajo se borran al terminar de editar y ESSD puede no estar montado. Muestra
   las fuentes offline y ofrece relink volviendo a pedir `media sources` (la alternativa de MEDIA), validando
   asset_id/session_id/sha256. Nunca cambies en silencio a otra revisión.
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
