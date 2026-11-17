# Windows: lo que nove y nüün repiten y qué subir aquí

Estado: **propuesta** para una sesión del core (2026-11-16). La escribió una sesión de nüün en el PC con Windows, al cerrar el paso 3 de su fase A, después de leer el código de las dos apps de Windows de la suite. No cambia ningún contrato ni ninguna pieza: dice qué se repite, qué decidir y en qué orden subirlo. Cada pieza entra en un cambio propio, con sus pruebas y su línea en el [CHANGELOG](../CHANGELOG.md) ([AGENTS](../AGENTS.md)).

Leído en: `nove` `896bc5d` (`apps/windows`, `scripts/windows`, `core/src/os.rs`) y `n-creator-utils` `5fa3bf8` (`native/windows`, `scripts/windows`, `engine/crates/nuun-media/src/interrupt.rs`). Antes de empezar, la sesión del core **debe** releer los dos repos (`git log`) por si cambiaron.

## 1. Resumen

| # | Pieza | Hoy | Propuesta | Condición |
| --- | --- | --- | --- | --- |
| 1 | Cancelar un trabajador en Windows | nüün: evento con nombre. El contrato pide método o archivo, más plazo de gracia y Job Object | **Decisión del usuario** (§2) y luego el cliente común de la §3 | Ahora: nüün ya lo usa en su ventana |
| 2 | Cliente de motor en C#: sobre v1, errores, trabajador de una vez | nüün `Nuun.Core/Engine`, nove `Nove.Core/EngineClient.cs` y `EngineHost.cs` | `windows/Nu9ve.Core/Engine` (§3) | Ya hay dos usuarios |
| 3 | Avisos del sistema | nove `Nove/Notifications.cs` | `windows/Nu9ve.WinUI/Notifications` (§4) | nüün lo necesita en el paso 5 de su fase A |
| 4 | Scripts de QA y empaquetado | Copias en los dos repos que ya se separaron | `scripts/windows/` (§5) | `ui-invoke.ps1` y `capture-window.ps1` ya; los demás, generalizados |
| 5 | Certificado de desarrollo | Dos sujetos distintos | **Decisión del usuario** (§6) | Antes de instalar las dos apps en una misma máquina limpia |
| 6 | Instancia única y roles del ejecutable | nüün copió el `Program.cs` de nove | `Nu9ve.WinUI.AppEntry` (§7) | Cuando una tercera app de Windows lo necesite, o junto con la §4 |
| 7 | Progreso en la barra de tareas | Nadie lo tiene | Nace aquí, en `Nu9ve.WinUI` (§8) | La primera app que lo pida (nüün, paso 5) |

## 2. Decisión abierta: cómo se cancela un trabajador en Windows

**Qué dice el contrato.** [`nu9ve-engine-v1` §6.2](../contracts/nu9ve-engine-v1.md#62-progreso-y-cancelación) y la decisión 4 de su §13 (del usuario, 2026-11-16): en Windows se cancela primero de forma cooperativa «con un método `<cosa>_cancel` o un archivo de cancelación que el motor vigila». Después hay un plazo de gracia, y al final el cliente corta con `TerminateJobObject` sobre el Job Object en el que arrancó el proceso. No se usa `GenerateConsoleCtrlEvent`.

**Qué hizo nüün** (`n-creator-utils` `5fa3bf8`, paso 3 de su fase A):

- La ventana crea un evento con nombre (`Local\nuun-cancel-<guid>`, de reinicio manual) y pasa el nombre al trabajador `nuun-engine rpc` en `NUUN_CANCEL_EVENT`.
- Para cancelar, la ventana lo señala. El motor lo consulta sin esperar (`WaitForSingleObject(…, 0)`) en los mismos puntos recuperables que SIGINT en macOS, anota el job `paused` y responde `cancelled`. `resume` lo continúa.
- Probado en Windows 11 con pruebas de Rust y de C# y con un recorrido por UI Automation.
- **No tiene** plazo de gracia ni Job Object. Si el motor se colgara, la ventana esperaría sin fin.

Es cooperativo, como pide el contrato, pero no es ninguna de las dos formas escritas. Opciones:

1. **Aceptar el evento con nombre en la §6.2** como tercera forma cooperativa, junto al método y al archivo. *Recomendada.*
   - nove ya avisa a su host con eventos con nombre (`HostIdentity.WindowQuitEvent`, `Local\nove-ui-quit-…`).
   - No deja archivos huérfanos tras una caída, desaparece con el último handle y queda en la sesión del usuario (`Local\`).
   - No necesita un canal persistente: sirve justo para los trabajadores de una vez, donde un método `_cancel` no tiene a quién llamar.
   - Cambiar el contrato es una regla nueva: la decide el usuario y sube la nota de la §13.
2. **Cambiar nüün a un archivo de cancelación**, como Clipper. El contrato no cambia. Hay que elegir dónde vive el archivo (bajo `--state-dir`), borrarlo al terminar y tolerar uno viejo tras una caída.

**En las dos opciones** falta lo mismo en el cliente de nüün, y lo puede dar la pieza común de la §3:

- arrancar el trabajador dentro de un Job Object con `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, como ya hace el motor de nove con sus hijos (`nove/core/src/os.rs`);
- un plazo de gracia tras pedir la pausa;
- después, `TerminateJobObject`, e informar `cancelled` sin dar nada por publicado.

En nüün eso es seguro: el diario deja el job reanudable aunque el proceso muera a media copia, lo que el recorrido de su fase A ya comprobó matando el trabajador.

**Del lado de Rust.** La guarda de nüün (`interrupt.rs`, la parte de Windows) es genérica: lee una variable, valida un prefijo, abre el evento y lo consulta. Hoy tiene un solo usuario: nove cancela con métodos dentro de su motor y Clipper con archivo. Por la regla de entrada se queda en nüün hasta que un segundo motor la use. Si el usuario elige la opción 1, conviene que nazca aquí (`nu9ve-engine::cancel`, con el prefijo como parámetro) en el mismo cambio del contrato.

## 3. Cliente de motor en C#: `windows/Nu9ve.Core/Engine`

Las dos apps hablan `nu9ve-engine-v1` desde C# con transportes distintos: nüün por stdio con líneas (`rpc-serve`, y `rpc` de una vez), y nove por marcos en un named pipe. El transporte se queda en cada app; lo demás se repite:

| Qué | nüün | nove | Común |
| --- | --- | --- | --- |
| Error del motor | `EngineFailure(code, message)` en `RpcConnection.cs` | `EngineException(code, message, retryable)` en `EngineClient.cs` | Un tipo con `code`, `message` y `retryable`, y los códigos de la [§7](../contracts/nu9ve-engine-v1.md#7-códigos-de-error) como constantes |
| Sobre v1 | `RpcConnection.Unwrap` | lectura del sobre en `EngineClient` (sólo v1 desde la 0.12.0.54) | `EngineEnvelope.Unwrap(JsonNode)` probado con [`data/engine-examples-v1.json`](../data/engine-examples-v1.json) |
| `engine_info` | se lee en `MethodByMethodEngine` | `hello` retirado por v1 | DTO con `contract`, `protocol`, `capabilities`, `ported`, `writes` |
| Trabajador de una vez | `EngineWrites.CallOnceAsync` y `CallCancellableAsync`: proceso oculto, petición por stdin en UTF-8 sin BOM, respuesta por stdout | `EngineHost.StartAsync`/`StopAsync` para su motor persistente (no de una vez) | `OneShotWorker`: Job Object con *kill on close*, cancelación cooperativa (§2), plazo de gracia y `TerminateJobObject` |
| Lector persistente | `EngineProcess.StartRust` + `ProcessOwner` | `EngineHost` + `--parent-pid` | Vigilar al padre y cerrar con el handle: comparar con [§8 ciclo de vida](../contracts/nu9ve-engine-v1.md#8-ciclo-de-vida-del-proceso) antes de unificar |

Pasos:

1. Escribir la pieza con pruebas propias en `windows/Nu9ve.Core.Tests`: los ejemplos del contrato y un trabajador falso (un `.exe` de prueba o `cmd /c`) que responda, se cuelgue y se cancele.
2. nüün la adopta en `Nuun.Core/Engine`; sus pruebas de contrato con el binario real deben seguir en verde (38 hoy).
3. nove adopta el tipo de error y el sobre en `Nove.Core`; su transporte de pipe no cambia.

Una trampa que ya costó tiempo: Windows PowerShell 5.1 antepone un BOM a la entrada estándar de un proceso hijo si `[Console]::InputEncoding` es UTF-8 con BOM, y el motor responde `invalid_request`. .NET 10 no lo hace. Conviene escribirlo en la guía de [Windows](platforms/windows.md) para los scripts de QA.

## 4. Avisos del sistema: `windows/Nu9ve.WinUI/Notifications`

`nove/apps/windows/Nove/Notifications.cs` (177 líneas) ya cumple casi todo [`nu9ve-notifications-v1`](../contracts/nu9ve-notifications-v1.md) en Windows. nüün lo necesita en el paso 5 de su fase A: un aviso al terminar con «Expulsar tarjeta» y «Ver resultado».

| Genérico (sube) | De cada app (se queda) |
| --- | --- |
| El rol `--deliver-toast <carpeta> --parent-pid <pid>` que muestra un aviso y sale, con el hilo que vigila al padre | El plan de entrega (outbox) del motor de nove |
| `AppNotificationManager`: `Register`/`Unregister` en la ventana, y `notifications_disabled` cuando el sistema lo bloquea (§3 del contrato) | El texto y los botones de cada aviso |
| Constructor de tres líneas y hasta dos botones, con `Tag`/`Group` para que un reintento reemplace (§4 y §5) | Los ids válidos (nove: `webalert-N`, `otcalert-N`; nüün: el UUID de un job) |
| Validación de los argumentos de activación con prefijos que da la app, como `AlertFrom`/`ValidAlert` (§4) | Qué se abre al hacer clic |

Pasos: subir lo genérico con pruebas de la validación de argumentos y del constructor (sin mostrar avisos reales); nove lo adopta sin cambiar lo que ve la persona; nüün lo usa en su paso 5.

## 5. Scripts de Windows: `scripts/windows/`

| Script | Estado entre los dos repos | Propuesta |
| --- | --- | --- |
| `ui-invoke.ps1` | Idéntico salvo la línea de origen | Subir tal cual |
| `capture-window.ps1` | Igual salvo el nombre de la clase de P/Invoke (`NoveWin`/`NuWin`) | Subir con un nombre neutro (`Nu9veWin`) |
| `package-msix.ps1` | Mismo origen; nüün añadió `-NoSign`/`-Sign` y lee la versión de `Directory.Build.props`; nove compila también su motor | Generalizar con parámetros (proyecto, fuente de la versión, sujeto del certificado, motor opcional) |
| `make-icons.ps1` | Mismo escritor de `.ico`; el dibujo es de cada app | Subir el escritor de `.ico` y de los PNG del MSIX; el dibujo sigue en cada app |
| `trust-dev-cert.ps1` | Sólo nove | Subir junto con la §6 |
| `real-click.ps1`, `measure-resources.ps1` | Sólo nove, genéricos | Subir cuando una segunda app los use |
| `qa-skins.ps1` | Sólo nüün; nove ya tiene skins desde `cee0684` | Subir con la página y los skins como parámetros |
| `qa-page-server.ps1`, `qa-telegram-fake.ps1`, `test-chrome-extension.ps1` | De nove | Se quedan |

Las apps los toman como cualquier pieza (`vendor/nu9ve-core.paths`) y dejan de llevar su copia. No hay versión de bash: son de Windows.

## 6. Decisión abierta: un certificado de desarrollo para la suite

`package-msix.ps1` firma con `CN=nove dev (local)` en nove y `CN=nu9ve dev (local)` en nüün. Para instalar las dos en una máquina hay que confiar en dos certificados. Opciones: un solo `CN=nu9ve dev (local)` y un solo `trust-dev-cert.ps1` para la suite (recomendada), o uno por app. El `Publisher` del manifiesto MSIX debe coincidir con el sujeto, así que nove tendría que reinstalar con el nuevo. Es una decisión del usuario y va antes de la firma e instalación que ambas apps tienen pendiente.

## 7. Instancia única y roles: `Nu9ve.WinUI.AppEntry`

`n-creator-utils/native/windows/Nuun/Program.cs` es una copia declarada del de nove: `Main` propio, `InitializeComWrappers`, `RedirectToExisting` con `AppInstance.FindOrRegisterForKey` antes de XAML, y roles por argumento ([windows-app-v1 §8](../contracts/windows-app-v1.md)). Lo común cabe en una clase que reciba la clave de instancia, la variable de QA y los roles. Puede esperar a una tercera app de Windows, o ir junto con la §4, porque `--deliver-toast` es un rol.

## 8. Progreso en la barra de tareas

Ninguna app lo tiene (lo propone nove: `docs/suite-alignment-windows.md` §3.3). Nace genérico aquí: `ITaskbarList3.SetProgressValue`/`SetProgressState` desde la misma medida global de `ActionProgress` (`overall_done`/`overall_total`), con el estado de pausa y error. La primera usuaria es nüün en el paso 5.

## 9. Orden propuesto

1. El usuario decide la §2 (cómo cancelar) y la §6 (certificado).
2. `scripts/windows/`: `ui-invoke.ps1` y `capture-window.ps1`. Es el cambio más chico y quita una copia de cada repo.
3. `Nu9ve.Core/Engine` (§3) con la cancelación decidida. Si es la opción 1 de la §2, en el mismo cambio que la nota del contrato.
4. `Nu9ve.WinUI/Notifications` (§4), antes del paso 5 de nüün.
5. Barra de tareas (§8) con el paso 5 de nüün.
6. `package-msix.ps1` generalizado y el certificado (§5 y §6), antes de firmar e instalar en una máquina limpia.
7. `AppEntry` (§7) cuando toque.

Al cerrar cada paso, registrar en [adopción por app](adoption.md) quién lo usa y volver a fijar las dos apps.
