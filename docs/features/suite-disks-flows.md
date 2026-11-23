# Discos de trabajo, conversaciones y sugerencias · diseño

Actualizado: 2026-11-23. Pedido del usuario del mismo día: con dos Macs (la M4 procesa Atlas en XSSD; la M1 explora en nove con ESSD) y MEDIA501/502 de archivo, nüün copia entre discos, respalda las conversaciones por dispositivo y sugiere sólo cuando hay algo nuevo. Se apoya en el core: [volumen](../../vendor/nu9ve-core/contracts/nu9ve-volume-v1.md) §7–§8, [datos](../../vendor/nu9ve-core/contracts/nu9ve-data-v1.md) §2.1 y §6, [agente](../../vendor/nu9ve-core/contracts/nu9ve-agent-v1.md), [intercambio](../../vendor/nu9ve-core/contracts/nu9ve-exchange-v1.md) y [varias máquinas](../../vendor/nu9ve-core/docs/suite-devices.md) §6.4–§6.5; y en nove A70–A76. Estado y evidencia en [STATUS](../STATUS.md#discos-de-trabajo-y-conversaciones--2026-11-23).

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
- **Falta:** la ventana (Discos → elegir carpeta o proyecto → «Copiar a ESSD»), el proyecto de Clipper con sus dependencias (`paths` desde `clipper.inspect`) y resumir lo copiado en el índice de nüün del destino.

## 2. Respaldo de conversaciones por dispositivo

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

## 3. Sugerencias al conectar

`suite_suggestions` (lectura) mira los discos montados y la configuración y devuelve sólo lo que tiene algo que hacer:

| Flujo | Cuándo aparece |
| --- | --- |
| `work_copy` | Dos discos de trabajo conectados y una raíz de otra app en uno (su `volume_path`) que no existe en el otro ni está en un recibo de copia hacia él |
| `conversations_backup` | El disco de respaldo elegido está conectado y una colección decidida no se respalda desde hace N días (o nunca) |
| `conversations_working` | El destino de la copia de trabajo está conectado y quedó atrás del último respaldo |

- Cada sugerencia tiene una clave estable (`flujo:volume_id[:…]`). «No volver a preguntar» la guarda en el catálogo local para ese disco y flujo (`suggestions_dismiss`). «Ahora no» la calla hasta el próximo evento: la app recuerda, en memoria, qué sugirió por montaje.
- Las SD siguen con su ventana de ingreso de siempre; esto no la repite.

## 4. Otras Macs, por el agente

- «Trabajar aquí» ya usa `lease_take`/`lease_release` (F1). Después de copiar un proyecto a un disco, la sugerencia en la otra Mac es «Trabajar aquí» en ese proyecto.
- Las conversaciones de otras máquinas no viajan por la carpeta de intercambio: van en el disco (respaldo o copia de trabajo). Por el agente sólo se ve su forma (`where`), como manda data §6.
- No se toca el agente instalado, el llavero ni los LaunchAgents: todo se prueba con `NU9VE_AGENT_DIR` y carpetas de QA.

## Decisiones del usuario

1. **Carpeta de las conversaciones en MEDIA:** `004_datos/conversaciones/<colección>/<device_id>/` (propuesta; al lado de `004_datos/clipper/transcripts`).
2. **Qué entra:** sólo historial (tabla de arriba). Abierto: las bases vivas de Codex (`thread_history*.sqlite`, 1,6 GiB en la M1) quedan **fuera** por omisión porque Codex las escribe mientras corre; entrarían copiando un snapshot (`VACUUM INTO`) o con Codex cerrado.
3. **Cuándo:** «Sugerir cada 7 días» por omisión; «Automático» sólo si lo eliges.
4. **Copia de trabajo:** a qué disco o carpeta (p. ej. ESSD `004_datos/conversaciones`), y si va con cada respaldo.
5. **Copias de MEDIA a un disco de trabajo** no entran en `work_copy` (son «Entregar trabajo»). Abierto si quieres un solo botón para las dos.
6. **Recibo también en el origen** de una copia entre discos: elegido sí (el origen sabe adónde fue). Dímelo si prefieres sólo el destino.
