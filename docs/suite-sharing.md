# Cómo se comparte código y contratos entre los repos de la suite

Estado: **decidido por el usuario el 2026-11-11, sin implementar**: repo `nu9ve/suite` con copia fijada en cada app (forma C de la §2). Siguen abiertas las preguntas 3 y 4 de la [§8](#8-lo-que-decide-el-usuario). Nació como propuesta el 2026-11-10, parte de la [auditoría Fable](audits/fable-audit-2026-11-10.md) ([C9](audits/fable-audit-2026-11-10.md#c9--los-contratos-de-la-suite-viven-en-el-repo-de-una-app)). Responde a la pregunta del usuario: ¿el repo común sería `n-creator-utils`?, ¿todos dependerían de tenerlo al desarrollar y de manejar versiones entre ellos?, ¿un solo repo con lo de Windows, Mac y Rust, o repetir en cada repo y coordinar sólo por contrato? Nada de esto está hecho; es la decisión que destraba varias colas.

## 1. Respuesta corta

- **No `n-creator-utils`.** Ése es el repo de nüün. Hoy guarda los contratos de la suite por accidente histórico, y por eso nove y curado «dependen» del repo de otra app.
- **Sí un repo propio y pequeño, `nu9ve/suite`**, donde lo compartido se **edita**: contratos, datos comunes y el código común de Mac, de Windows y de Rust, en carpetas separadas del mismo repo.
- **Ninguna app necesita tener ese repo clonado para compilar ni para trabajar.** Cada app lleva dentro una **copia fijada** de lo que usa, con el commit del que salió y una lista de hashes, y un script que la actualiza y la comprueba. Es lo que Fractos ya hace con el contrato de la web (`ios-fractos`: `Contracts/`, `scripts/sync-contracts.sh`).
- **Las versiones no se negocian entre apps.** Cada app dice de qué commit de la suite es su copia y la sube cuando le conviene. Un cambio en la suite no rompe a nadie hasta que esa app sincroniza.
- **No es «repetir en cada repo».** Repetir a mano y coordinar por contrato fue lo correcto para arrancar («replicar primero»). La copia fijada es el paso siguiente: la misma copia, pero idéntica al origen y comprobada por máquina.

## 2. Las tres formas, comparadas

| | A · Repetir y coordinar por contrato (hoy) | B · Repo común como dependencia de paquete | **C · Repo común + copia fijada** |
| --- | --- | --- | --- |
| Qué es | Cada app tiene su copia a mano, con comentario de origen | Cargo por `git`, SwiftPM por URL, NuGet o submódulo para C# | Cada app guarda una copia exacta del origen, con commit y hashes |
| Clonar una app y compilar sin nada más | Sí | No: baja dependencias, y un repo privado pide credenciales en cada máquina y en CI | Sí |
| Compilar sin red (`--offline`, como hoy en `engine/`) | Sí | Sólo con la caché ya poblada | Sí |
| Una sesión de un agente en el repo de la app puede leer el código compartido | Sí | No: vive en la caché del gestor | Sí |
| Mismo mecanismo en Rust, Swift, C# y datos | — | No: tres mecanismos, y C# no tiene dependencias por `git` | Sí: uno |
| Las copias divergen | **Sí**, es el problema actual | No | No: el gate falla si alguien toca la copia |
| Actualizar | A mano, pieza por pieza | Cambiar una versión | Correr un script; llega como un diff revisable |
| Arreglo en lo compartido | En cada app, y luego recordar las demás | En el repo común | En el repo común, y sincronizar |
| Cuándo conviene | Una pieza con una sola copia que sigue cambiando | Varios equipos, muchos consumidores, publicación frecuente | Una persona, varias máquinas, sesiones por repo, repos privados |

**Recomendación: C.** Las razones son las de esta suite, no generales:

- Se trabaja por repo, con sesiones que sólo ven ese repo. El PC de Windows no tiene la mitad de los repos; con B, una sesión ahí no podría leer `NuMacShell` ni el contrato de tema sin clonar y autenticar otro repo.
- Los repos son privados. B obliga a configurar credenciales para el gestor de paquetes en cada máquina y en CI.
- C# no tiene una forma nativa de depender de un `git`, así que B ya sería dos mecanismos distintos.
- La regla del repo es compilar sin descargar nada, con dependencias fijadas y anotadas. C la conserva.
- Ya está probado en la suite: Fractos lo usa para el contrato de la web, con `--check` y `--strict`.

C no es irreversible. Si un día hay más gente o más consumidores, las mismas carpetas se vuelven dependencias de paquete cambiando una ruta por una URL.

## 3. Qué vive en `nu9ve/suite`

```text
suite/
├─ contracts/     macos-app, windows-app, nu9ve-theme, nu9ve-interaction, nu9ve-mcp,
│                 nu9ve-sound, nu9ve-notifications (y los que vengan: protocolo, identidad de volumen)
├─ docs/          suite.md (mapa), platforms/ (windows, os-integration, linux), auditorías
├─ data/          theme-vectors-v1.json · sound/ (referencia medida y paquete de sonidos)
├─ rust/          workspace: nu9ve-json, nu9ve-volumes, nu9ve-time
│                 (después: nu9ve-ipc, nu9ve-mcp, nu9ve-sound)
├─ swift/         Package.swift: NuMacShell (después: roles de tema, motor de sonido)
├─ windows/       Nu9ve.Core, Nu9ve.WinUI y sus pruebas
├─ scripts/       sync-suite (sh y ps1), scripts de QA de Windows
└─ CHANGELOG.md
```

Un solo repo con las tres plataformas está bien: son carpetas independientes que cada app toma por separado. La ventaja de tenerlas juntas es que un contrato y su implementación en Swift, C# y Rust cambian en el mismo commit.

**Qué no entra:**

- Nada con el nombre de una app: `nuun-*`, los contratos de media, los de nove, el motor de audio de curado.
- El contrato del API de Fractos y el motor de sonido de la web: son del repo de la web (`nu9ve.xyz`). La suite toma de ahí datos generados ([§6](#6-el-caso-del-sonido-y-de-la-web)), no el código.
- Lo que todavía tiene una sola copia y sigue cambiando (el pipe y el host de bandeja de nove, el MCP): eso se replica primero.

**Regla de entrada**, la de siempre afinada: una pieza entra cuando dos apps la usan igual, o cuando ya nació genérica y sin tipos de una app. `NuMacShell` cumple lo segundo (el gate de Fractos impide que importe nada de Fractos); `nu9ve-json` y `nu9ve-volumes` también.

## 4. Cómo lo consume una app

Dentro de cada repo de app, una carpeta `vendor/suite/` con sólo lo que esa app declara que usa:

```text
nove/
└─ vendor/suite/
   ├─ SOURCE              repo=nu9ve/suite · commit=… · date=…
   ├─ MANIFEST.sha256     un hash por archivo copiado
   ├─ contracts/          los contratos, para leerlos sin salir del repo
   ├─ data/theme-vectors-v1.json
   ├─ rust/…              sólo los crates que nove usa
   └─ windows/Nu9ve.Core, Nu9ve.WinUI
```

| Ecosistema | Cómo se referencia |
| --- | --- |
| Rust | `nu9ve-json = { path = "../vendor/suite/rust/nu9ve-json" }` |
| Swift | `.package(path: "vendor/suite/swift")` |
| C# | `<ProjectReference Include="..\vendor\suite\windows\Nu9ve.Core\Nu9ve.Core.csproj" />` |
| Datos y contratos | Se leen del archivo; las pruebas los usan directamente |

El script, calcado del de Fractos:

| Comando | Qué hace | Cuándo |
| --- | --- | --- |
| `sync-suite --check` | Comprueba que la copia coincide con su manifiesto: nadie la editó | En el gate de cada cambio |
| `sync-suite --strict` | Además falla si la suite avanzó | Antes de una release |
| `sync-suite <commit>` | Trae ese commit de la suite y reescribe `SOURCE` y el manifiesto | Al actualizar, en un cambio propio |
| `sync-suite --from <carpeta>` | Copia desde un checkout local de la suite, para probar un cambio antes de subirlo. Marca la copia como sucia y el gate la rechaza hasta fijar un commit | Sólo mientras se desarrolla |

Reglas:

- **La copia no se edita.** Un arreglo va primero al repo de la suite y después se sincroniza. Es la misma regla de `Contracts/` en Fractos.
- **Actualizar es un cambio propio**, sin mezclar con otra cosa, como hoy una dependencia en nove o curado.
- Los enlaces entre repos dejan de hacer falta: cada app tiene los contratos en su propia carpeta. Eso resuelve de paso las rutas de una Mac concreta ([auditoría C6](audits/fable-audit-2026-11-10.md#c6--enlaces-entre-repos-con-rutas-de-una-mac)).

## 5. Versiones y coordinación

- **En la suite:** un `CHANGELOG.md` con una entrada por cambio, diciendo qué pieza y si rompe algo. Los contratos llevan su versión en el nombre (`-v1`). Una etiqueta con fecha sólo cuando haga falta nombrar un punto.
- **En cada app:** `vendor/suite/SOURCE` es la versión. STATUS puede decirlo en una línea («suite en `abc1234`»).
- **Un cambio que rompe:** se hace en la suite con su prueba; cada app lo adopta al sincronizar, a su ritmo. Si el cambio es de un contrato, sube a `-v2` y las dos versiones conviven mientras las apps migran.
- **Quién va atrasado:** `sync-suite --strict` lo dice por app. Las tablas de adopción de cada contrato siguen siendo el resumen humano.
- **Dónde se prueba lo compartido:** una vez, en el repo de la suite, en los tres sistemas (Rust en macOS, Windows y Linux; Swift en macOS; C# en Windows). Las apps prueban que lo usan bien, no que funciona.

Ejemplo de un cambio completo, «progreso en la barra de tareas para todas las apps de Windows»:

1. Se añade a `windows/Nu9ve.WinUI` en la suite, con su prueba y su línea en el CHANGELOG.
2. En nüün: `sync-suite <commit>`, se revisa el diff, se usa.
3. nove lo recibe cuando sincronice. Hasta entonces sigue compilando igual que ayer.

## 6. El caso del sonido y de la web

El usuario quiere que toda la suite use los mismos sonidos, los del motor v2, cuya versión real y más probada está en la web (`nu9ve.xyz`, que no está en todas las máquinas).

La copia fijada resuelve esto en dos saltos, sin que ninguna app necesite el repo de la web:

```text
nu9ve.xyz (motor de la web) ──genera en la Mac──▶ suite/data/sound/ ──copia fijada──▶ cada app
```

- En la Mac, con el repo de la web: se genera la **referencia medida** y el **paquete de sonidos**, y se guardan en `suite/data/sound/` con el commit de la web del que salieron.
- Cada app toma de ahí lo que usa. El PC de Windows tiene los mismos sonidos que la Mac sin haber visto nunca la web.
- El motor de Swift (`FractosSoundV2`) se muda a `suite/swift/` cuando una segunda app de Mac lo adopte; Fractos pasa a consumirlo como las demás.

Detalle en [sonido de tema v1 §4](contracts/nu9ve-sound-v1.md#4-cómo-llegan-los-sonidos-a-cada-app). Una alternativa de largo plazo, sólo si mantener tres puertos del mismo motor (web, Swift, y uno futuro para Windows y Linux) se vuelve una carga: un único motor en Rust compilado a nativo y a WebAssembly para la web. No se recomienda ahora: el de la web es el probado.

## 7. Migración por pasos

Ningún paso deja una app sin compilar, y cada uno cabe en una sesión.

| Paso | Qué se mueve | Quién lo consume después |
| --- | --- | --- |
| 1 | Se crea el repo con los contratos de suite, `suite.md`, `platforms/` y los vectores de tema. Sólo documentos y datos | Todas, como lectura. `n-creator-utils` deja un índice que apunta a su copia |
| 2 | El script `sync-suite`, generalizando el de Fractos, con su gemelo en PowerShell | Todas |
| 3 | `rust/`: `nu9ve-json`, `nu9ve-volumes`, `nu9ve-time` | nüün; curado cuando cambie su `identity.rs` por el crate |
| 4 | `windows/`: `Nu9ve.Core`, `Nu9ve.WinUI` | nüün; nove (su trabajo de tema esperaba esta decisión) |
| 5 | `swift/`: `NuMacShell`, desde `ios-fractos` | Fractos, nüün; SoundShop y neuf al tocarlos |
| 6 | `data/sound/` y, después, el motor de sonido de Swift | nüün Windows primero |

`n-creator-utils` vuelve a ser sólo nüün: su motor, sus apps y los contratos de media.

## 8. Lo que decide el usuario

1. ~~¿Se crea `nu9ve/suite`?~~ **Sí** (2026-11-11). Falta confirmar el nombre y que sea privado al crearlo.
2. ~~¿Copia fijada o dependencias de paquete?~~ **Copia fijada** (2026-11-11).
3. ¿`NuMacShell` y, después, el motor de sonido salen de `ios-fractos` y Fractos los consume como las demás?
4. ¿Los documentos de la suite se quedan en español, como hoy? (curado documenta en inglés.)

Brief para ejecutarlo: [S5](audits/session-briefs-2026-11-10.md#s5--decidir-y-crear-el-repo-de-la-suite).
