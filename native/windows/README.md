# nüün para Windows

Cliente WinUI 3 de nüün. Con el motor de Rust junto a la app, lee los discos de este PC, registra destinos y hace el ingreso completo: planea (`plan_document`), respalda con verificación (`apply`), pausa en un punto recuperable y reanuda (`resume`). Sin el motor, o con `--maqueta`, muestra datos ficticios rotulados como maqueta. **Aún no se probó con una SD ni un disco USB reales** (paso 4 de la fase A en el [PLAN](../../PLAN.md#motor-en-rust--ruta)). Reglas: [windows-app-v1](../../vendor/nu9ve-core/contracts/windows-app-v1.md) · guía y aprendizajes: [Windows en la suite](../../vendor/nu9ve-core/docs/platforms/windows.md) · estado: [STATUS](../../docs/STATUS.md).

| Proyecto | Contenido | Depende de |
| --- | --- | --- |
| `Nu9ve.Core`, `Nu9ve.WinUI` | La base de Windows de la suite, desde la copia fijada en [`vendor/nu9ve-core/windows`](../../vendor/nu9ve-core/windows/README.md). No se edita aquí: el cambio va a `nu9ve/core` y se sincroniza. Compila en `vendor/.nu9ve-core-build/` | Windows App SDK |
| `Nuun.Core` | Lo propio de nüün: su identidad de tema (`Theme/NuunTheme.cs`: ámbar, presets suizos, Cristal y Gótico, textos de Apariencia), transporte JSON con `rpc-serve` y trabajadores `rpc`, DTOs (plan revisado, progreso desde `job`), maqueta (`Fixtures/*.json` con la forma real de `snapshot` y `recipe`), catálogo de atajos, modos de ventana | `Nu9ve.Core` |
| `Nuun` | La ventana: Inicio, Ingreso (Entrada → Flujo → Resultado), Flujos, Biblioteca, Ajustes, Acerca de | todo lo anterior |
| `Nuun.Setup` | `NuunSetup.exe`: instalador con identidad (forma de SD, skins, `PackageManager`, `--quiet`) | `Nu9ve.WinUI`, `Nuun.Core` |
| `Nuun.Core.Tests` | 38 pruebas: la identidad de tema de nüün (su ámbar, presets en cada skin y superficie), atajos, ventana, transporte, maqueta y contrato con el motor de Rust, incluido el ingreso de una carpeta temporal con pausa y reanudación. La matriz de contraste, los vectores y el instalador se prueban en `nu9ve/core` (`windows/Nu9ve.Core.Tests`) | `Nuun.Core` |
| `Package` | Plantilla del manifiesto MSIX (`app.nu9ve.media`, la misma identidad que en Mac) | — |

Mismo stack y versiones que nove (`.NET 10`, Windows App SDK 1.8.260921001, xUnit 2.9.3); no se añadió ningún paquete nuevo.

## Compilar, probar y abrir

```powershell
dotnet test native\windows\Nuun.Core.Tests
dotnet build native\windows\Nuun -c Debug
$env:NUUN_QA_ROOT = "$env:TEMP\nuun-qa\lib"; native\windows\Nuun\bin\Debug\net10.0-windows10.0.22000.0\win-x64\Nuun.exe
```

Con `NUUN_QA_ROOT` las preferencias de la ventana van bajo esa raíz y cada ejecución es una instancia aparte. Sin ella, Debug guarda en `%LOCALAPPDATA%\nuun\Desarrollo`. Argumentos: `--page ingest|flows|library|settings|about`, `--skin glass|classicMac|win9x|terminal|gothic` (sólo con raíz de QA), `--compact` y `--maqueta` (datos ficticios aunque el motor de Rust esté junto a la app; sin ella, la ventana muestra los datos reales de este PC y no simula respaldos).

Motor de Rust: si `engine\target\debug\nuun-engine.exe` existe al compilar, se copia junto a la app (`engine\nuun-engine.exe`). La ventana lee por un `rpc-serve` persistente; cada escritura (`register_volume`, `discover`, `plan_document`, `apply`, `resume`) corre en su propio proceso `rpc`, como en la Mac. Ver [engine](../../engine/README.md#desde-la-ventana-de-windows). Python (`NUUN_PYTHON` y `NUUN_ENGINE`) sólo lee `snapshot` y `recipe`; con él, planear responde `not_ported`.

## Ingreso con el motor real

- **Fuente:** una tarjeta montada que el motor lista, u «Otra carpeta…» (selector del sistema o ruta escrita). Una carpeta se respalda y se conserva: sólo se libera la raíz de una tarjeta montada.
- **Destinos:** los registrados y en línea, por papel. Sin disco de trabajo, el plan es de sólo respaldo y se confirma **en cada operación**: la casilla se reinicia al elegir otra fuente o empezar otra ingesta. Si hay un disco de trabajo registrado pero desconectado, la copia para editar queda `pending`; si no hay ninguno, `skipped`.
- **Revisar plan:** `plan_document`. Flujo muestra el plan del motor por unidad; la ventana no calcula nada. Cambiar fuente, destinos u opciones invalida el plan.
- **Respaldar:** `apply` con el documento y el digest revisados, sin cambios. Resultado lee `job` cada medio segundo y muestra la medida global del motor (`overall_done`/`overall_total`), el archivo en curso y el registro.
- **Pausar:** la ventana crea un evento con nombre (`Local\nuun-cancel-…`), se lo pasa al trabajador en `NUUN_CANCEL_EVENT` y lo señala. El motor se detiene en el siguiente punto recuperable, anota el job `paused` y responde `cancelled`. El proceso nunca se mata, así que el job no queda como activo. **Por cambiar (decisión del 2026-11-26):** el motor ya acepta el archivo de cancelación del contrato (`NUUN_CANCEL_FILE`, ruta absoluta que la ventana crea para cancelar). La ventana debe pasarse a ese archivo, arrancar el trabajador en un Job Object y cortarlo con `TerminateJobObject` tras el plazo de gracia ([PLAN](../../PLAN.md#motor-en-rust--ruta)). El evento sigue aceptado mientras tanto.
- **Reanudar:** en Resultado tras una pausa o un error, y en Biblioteca → Actividad para los jobs `paused` o `failed` y los cortados a media copia, también después de cerrar la app o de que muera el trabajador. Lo ya verificado no se vuelve a leer.

Los vectores de tema compartidos se regeneran en `nu9ve/core` ([windows/README](../../vendor/nu9ve-core/windows/README.md)), no aquí.

## QA visual

```powershell
powershell -File scripts\windows\qa-skins.ps1 -Out "$env:TEMP\nuun-qa\shots"
powershell -File scripts\windows\qa-skins.ps1 -Skins win9x -Pages ingest -Compact
```

Cada skin × página se captura en una raíz temporal que se borra al terminar. `scripts\windows\ui-invoke.ps1 -ProcessId <pid> -List` recorre el árbol de UI Automation (el de Narrador) y `-Name "Revisar plan"` invoca un control. Las capturas sirven para revisar a mano: no reemplazan las del README ([capturas del producto](../../docs/development.md#capturas-del-producto)).

## Paquete y Setup

```powershell
powershell -File scripts\windows\package-msix.ps1 -NoSign
native\windows\Nuun.Setup\bin\Debug\net10.0-windows10.0.22000.0\win-x64\NuunSetup.exe
```

`-NoSign` publica, arma el MSIX y valida el manifiesto con `makeappx`, sin tocar certificados. `-Sign` crea, si falta, el certificado de desarrollo en `CurrentUser\My` y firma. Confiar en ese certificado requiere un paso aparte como administrador, que no está automatizado aquí. Sin `--package`, el Setup es una demostración rotulada: `--package dist\windows\nuun-<v>-x64.msix` instala de verdad y `--quiet` lo hace sin ventana (códigos 0/1/2/3).

Instala tú, desde el Explorador o tu terminal: una app agente empaquetada como MSIX virtualiza lo que sus procesos hijos escriben en `AppData` (lección de curado).

## Límites

El ingreso se probó sólo con carpetas locales en una raíz de QA (NTFS). Faltan la SD y el disco USB reales, expulsar, la confirmación de limpieza de cámara, el progreso en la barra de tareas y el aviso al terminar (pasos 4 y 5 de la fase A). Sin firma ni instalación probada, sin recordar el marco por modo. `TextBox` y `ComboBox` conservan el estilo del sistema dentro de los skins de época. Las fuentes OFL de los skins siguen pendientes, igual que los sonidos de tema, los badges con capa Ctrl/Shift y el inglés. Alto contraste, escalas 125/150/200 % y Narrador escuchado no se revisaron a mano.
