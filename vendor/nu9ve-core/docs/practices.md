# Prácticas comunes de las apps

Estado: **guía** (2026-11-22). Reúne lo que varias apps ya hacen igual, leído en sus repos ese día. Es la sesión U8 del [plan](suite-adoption.md#sesiones-de-unificación). No es un contrato y **no inventa reglas**: donde un contrato manda, enlaza a él, y donde las apps hacen cosas distintas, lo dice en «Diferencias» sin elegir. Si una app cambia cómo hace algo, cambia su fila aquí.

## 1. QA aislada

La regla es de [macos-app-v1 §10](../contracts/macos-app-v1.md#10-validación): la app tiene un modo de QA que no lee ni escribe datos, preferencias ni recientes de producción. Lo que tienen las apps hoy:

| App | Cómo se aísla | Dónde |
| --- | --- | --- |
| nüün | `NUUN_QA_ROOT` (y `NUUN_QA_SCRIPT`) | Motor y app |
| nove | `NOVE_QA_ROOT`, más `NOVE_QA_ENGINE`, `NOVE_QA_MCP_CONFIRM` y `NOVE_QA_TOAST_LOG` | `dev-mac.sh` abre la app con su biblioteca en `.qa/` ([nove `docs/development.md`](../../nove/docs/development.md)) |
| SoundBender | `SOUNDSHOP_QA_ROOT`, más `SOUNDSHOP_QA_SHELL`, `SOUNDSHOP_QA_KEEP` y `SOUNDSHOP_QA_EXPECT_OPEN` | App; las copias de QA son otro bundle id («SoundBender QA.app») |
| Stalker | `STALKER_QA_ROOT` (ruta absoluta), que hace de carpeta de inicio | README de Stalker |
| neuf | `NEUF_QA_SERVICE_HOME` y `NEUF_QA_ENGINE_MODE` | Sus scripts `qa_*.py` |
| kyu | Un argumento, `--qa-snapshot DIR`, con fixtures | STATUS de kyu |
| curado | Sólo en pruebas: `CURADO_TEST_DB` y `CURADO_TEST_DEVICE` | Pruebas de Rust |

Lo que comparten las que tienen variable: el nombre sigue `<APP>_QA_<COSA>`, y la carpeta reemplaza **toda** la raíz de datos de la app, no un archivo suelto. Los contratos de la suite siguen la misma forma para lo que es de la suite: `NU9VE_ENGINES_DIR` ([registro §1](../contracts/nu9ve-registry-v1.md#1-dónde)), `NU9VE_DEVICE_FILE` ([dispositivo §1](../contracts/nu9ve-device-v1.md#1-dónde)) y `NU9VE_QA_MCP_CONFIRM` ([MCP de la suite §5](suite-mcp.md#5-niveles-y-permiso-por-sesión)). En QA, una app no escribe en las carpetas de la suite de la persona.

**Diferencias:**
- **kyu** aísla con un argumento y no con una variable.
- **curado** sólo aísla en pruebas: su app no tiene modo de QA.
- **SoundBender** todavía usa el nombre viejo, `SOUNDSHOP_`.

## 2. Instalar en la Mac

Todas las apps se instalan con un script del repo, sin instalador, en la carpeta de aplicaciones del usuario para que Spotlight y Raycast las encuentren:

| App | Script | Destino | La versión anterior | La app abierta |
| --- | --- | --- | --- | --- |
| nüün | `scripts/install_media_app.py` | `~/Applications/nuun.app` (nombre ASCII, se muestra nüün) | Se guarda fuera de Aplicaciones, en `~/Library/Application Support/n-creator-utils/app-backups/`, para que los lanzadores no la indexen dos veces | — |
| curado | `scripts/install-local.ts` | `~/Applications` | Se aparta mientras se promueve la nueva y vuelve si algo falla | — |
| SoundBender | `scripts/install.sh` | `~/Applications` (`SOUNDBENDER_INSTALL_DIR` para otra) | Se aparta en una carpeta de preparación y vuelve si falla | Se cierra por bundle id; las copias de QA no se tocan |
| kyu | `scripts/install.sh` | `~/Applications` (`KYU_INSTALL_DIR`) | Se borra después de instalar | Nunca cierra un Kyū abierto |
| Stalker | `scripts/install.sh` | `~/Applications` (`STALKER_INSTALL_DIR`) | No se guarda: se copia encima | Se cierra por bundle id |
| nove | A mano ([nove `docs/development.md`](../../nove/docs/development.md)) | `/Applications/Nove.app` | Se respaldan antes **la app y su biblioteca**, en `~/Library/Application Support/nove-update-backups/<fecha>` | Se sustituye con la app cerrada |

Lo que comparten casi todas:
- **La copia nueva se prepara entera** (`ditto` o una copia de la carpeta) antes de tocar la instalada.
- **La instalada se cambia con un rename.** Si falla, vuelve la anterior.
- **Nunca se tocan los datos de la persona** al instalar.

nove es la única que además respalda la biblioteca, porque su versión de datos sube con la app.

**Diferencias:**
- Stalker no guarda la versión anterior.
- kyu la borra.
- nove instala en `/Applications` en vez de `~/Applications`, y a mano.

## 3. El gate local

Decisión del usuario del 2026-11-16 ([AGENTS](../AGENTS.md#sesiones-que-trabajan-en-la-suite), [CI sólo a mano](suite-adoption.md#ci-sólo-a-mano)): **el CI no corre en cada push**. Las pruebas se corren en local antes de cada commit, con un solo comando por repo:

| App | Gate | Incluye `sync-core.sh --check` |
| --- | --- | --- |
| kyu | `scripts/validate.sh`: Rust (formato, lint y pruebas), el framework del motor y Swift | Sí, primero |
| SoundBender | `scripts/validate.sh`: motor en Rust (formato, lints, dorados), Swift; `cargo-deny` si está instalado | Sí, primero |
| Stalker | `scripts/check.sh` | Sí |
| curado | `bun run check` (TypeScript y pruebas), `bun run check:rust` y `bun run core:check` | Sí, como comando propio |
| nüün | Su gate de motor y app, con dorados de Python y Rust | Sí |
| core | `scripts/check-swift.sh`, `scripts/test-sync-core.sh`, el workspace de Rust y los `--check` de los ejemplos | — |

Lo que comparten:
- **Un comando por repo** que corre todo lo que el repo prueba.
- **`sync-core.sh --check`,** en el gate o al lado.
- **Rust con `clippy -D warnings`** y el toolchain del CI de cada repo, por su ruta ([AGENTS](../AGENTS.md#sesiones-que-trabajan-en-la-suite)).
- **Un fallo ajeno al cambio** se reporta y no se arregla en el mismo commit.

## 4. Lo compartido y su copia

Las reglas son las de [AGENTS](../AGENTS.md) y [cómo se comparte](suite-sharing.md#4-cómo-lo-consume-una-app). Aquí, sólo cómo las cumplen las apps:

- **Fijar el core es un commit propio,** sin mezclar con otro trabajo. nüün y Clipper lo titulan «Fijar nu9ve/core en `<hash>`», y nove «Core fijado en `<hash>`».
- **La copia no se edita.** Si algo compartido falla, el arreglo va al core y la app vuelve a fijar.
- **Mudar una pieza al core:** el core recibe la pieza con su prueba y su línea en el CHANGELOG. La app de origen deja el enlace y fija la primera copia en el mismo paso (así se hizo con `nu9ve-json`, `NuMacShell`, `nu9ve-volumes` y `nu9ve-midi`).
- **CHANGELOG por pieza** en el core. curado y SoundBender tienen además el suyo en `docs/CHANGELOG.md`.

## 5. Al cerrar una sesión

Lo pide [AGENTS](../AGENTS.md#sesiones-que-trabajan-en-la-suite) y lo hacen las sesiones de las apps: un informe que la sesión del core pueda integrar. Lleva:
- los commits, con su repo y su hash;
- qué se verificó y con qué comando;
- qué falló y si ya fallaba antes;
- las decisiones tomadas o abiertas;
- qué falta.

La sesión del core lo comprueba en el repo antes de registrarlo en [adopción](adoption.md). Por ejemplo, revisa los marcadores y los índices sobre los discos con la referencia del core.

## 6. Mantener esta guía

Una fila cambia cuando cambia lo que hace la app, con su línea en el CHANGELOG. Si una diferencia de las de arriba se vuelve un problema, la solución es una regla en un contrato (que sube de versión si rompe), no un cambio aquí.
