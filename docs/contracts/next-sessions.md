# Próximas sesiones: Clipper y luego Stalker

Escritos el 2026-10-25 para correrlos uno después del otro. **Primero Clipper**:
- desbloquea editar `debloateando-mi-tele` hoy;
- deja al consumidor tolerante a cualquier nombre de bundle antes de que Stalker cambie su naming en origen.

Contrato: [media-naming-v1](media-naming-v1.md), [media-library-v1](media-library-v1.md).

## 1. Clipper (`~/code/ai/clipper`)

```text
Contexto: n-creator-utils ya respalda y prepara media con un naming fechado. Lee
/Users/nu9ve/code/nu9ve/n-creator-utils/docs/contracts/media-naming-v1.md (sobre todo "Identidad" y
"Adopción propuesta: Clipper") y media-library-v1.md. No edites ese repo; reporta allí sólo vía tus docs/ACK.

Objetivo de hoy: poder editar el proyecto "debloateando mi tele" desde
/Volumes/ESSD/001_proyectos/e64_raw/debloateando-mi-tele/
Contiene 7 clips Sony (26OCT24-194650_SONYA6400_08-24_MX-DEBLOATEANDO-MI-TELE[].MP4 …) y 3 sesiones
Stalker con carpeta renombrada (26OCT24-195850_STALKER_03-48_MX-DEBLOATEANDO-MI-TELE[].stalker …), intercaladas
por hora. Ya comprobé que clipper_tui.stalker.expand_inputs sobre esa carpeta devuelve los 10 en orden de
captura. Los internos de cada bundle (manifest.json, screen.mov, mic.wav, sources/, events.ndjson) no cambiaron.

Tareas:
1. Recorre el flujo completo con esa carpeta: importar, sync con reloj Stalker, transcripción, overlays, render
   y export. Busca roturas por `[]` en rutas (filtros ffmpeg como subtitles=/movie=, listas concat, glob(),
   shell) y por carpetas .stalker que no se llamen Stalker_*. Corrige con pruebas que usen copias en fixtures
   temporales con esos nombres; nunca escribas dentro de /Volumes/ESSD ni de los bundles.
2. Ningún código debe inferir sesión, orden ni duplicado por el patrón Stalker_*: valida por manifest.json
   (producer, sessionID).
3. Guarda sessionID (ya existe stalker_session_id) y, si es barato, un hash o tamaño por fuente. Cuando la raíz
   de un proyecto no exista (las copias de trabajo se borran al terminar de editar), muestra la fuente como
   offline y ofrece relink a otra carpeta, validando sessionID/tamaño. No hagas migraciones automáticas.
4. Disco interno: quedan ~5.7 GB libres. Propón o configura que el proyecto y los renders de este video vivan
   en ESSD (por ejemplo /Volumes/ESSD/001_proyectos/e64/debloateando-mi-tele/), no en ~/Downloads, y avisa
   antes de renders grandes sin espacio.
5. Deja creado o listo el proyecto Clipper de "debloateando mi tele" con esas 10 fuentes en orden, y dime cómo
   abrirlo.

Fuera de alcance: importar creator-media-handoff/1 (siguiente iteración), renombrar media, tocar
n-creator-utils. Al terminar: corre tus pruebas, documenta en tus docs qué adoptaste del contrato (ACK) y qué
falta, y dame un resumen con los cambios y cualquier limitación encontrada con los nombres nuevos.
```

## 2. Stalker (`~/code/ai/stalker`)

```text
Contexto: n-creator-utils archiva sesiones Stalker y las renombra con un naming fechado para que se
intercalen con clips de cámara. Lee /Users/nu9ve/code/nu9ve/n-creator-utils/docs/contracts/media-naming-v1.md
("Identidad", "Estado de archivo para productores", "Adopción propuesta: Stalker"). stalker-clipper/1 sigue
siendo tu autoridad de captura; no cambies reloj, pistas ni internos del bundle. No edites n-creator-utils.

Tareas:
1. Proyecto y país opcionales por sesión (o en el preset), guardados como metadata opcional versionada del
   manifest. Sin proyecto, todo sigue como hoy.
2. Con proyecto: al finalizar, cuando ya se conoce la duración y ANTES de marcar status=completed, renombra
   la carpeta de forma atómica a YYMONDD-HHMMSS_STALKER_MM-SS_PAIS-PROYECTO[].stalker (inicio = createdAt en
   hora local, duración = manifest.duration redondeada a segundos, slug en mayúsculas). Un bundle completed
   nunca se renombra ni se reescribe después. Prueba de contrato: createdAt 2026-10-25T02:58:50Z,
   duration 227618267500/1e9 s, MX, "debloateando mi tele", TZ America/Hermosillo →
   26OCT24-195850_STALKER_03-48_MX-DEBLOATEANDO-MI-TELE[].stalker
3. Biblioteca configurable y durable (default fuera de ~/Downloads); las sesiones viejas se quedan donde están.
   Muestra el espacio libre: el disco interno tiene ~5.7 GB libres y una sesión larga ocupa ~1 GB.
4. "Liberar sesiones archivadas": ejecuta `python3 /Users/nu9ve/code/nu9ve/n-creator-utils/nu9ve.py media
   captures <biblioteca>` (JSON {ok,data:{sessions:[{path,session_id,status,archived}]}}). Sólo las sesiones
   con archived=true pueden ofrecerse, por selección explícita del usuario, y van a la Papelera (recuperable).
   Nunca borres por un evento ni sin esa señal; si el comando falla o no existe, no ofrezcas liberar.
5. Actualiza docs/clipper-contract.md: el nombre de carpeta no es identidad; Clipper valida por manifest.

Al terminar: pruebas (incluida la de contrato), documenta lo adoptado/ACK y limitaciones, y dame un resumen.
```

## Después

Cuando ambas terminen, vuelve a n-creator-utils para:
- actualizar el estado de adopción en [README](README.md) y [STATUS](../STATUS.md) con lo que reporten;
- quitar `relayout` del flujo diario si Stalker ya nombra en origen (queda para material viejo).
