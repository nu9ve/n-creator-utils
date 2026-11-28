# Perfiles, páginas de una app y eventos de acción: qué subir al core

Estado: **propuesta** para una sesión del core (2026-11-27). La escribió una sesión de SoundBender a pedido del usuario, al planear sus perfiles y su llegada a [`nu9ve-surface-v1`](../contracts/nu9ve-surface-v1.md) como segunda app con controlador (C4). No cambia ningún contrato ni ninguna pieza: dice qué falta, por qué y en qué orden. Cada pieza entra en un cambio propio, con sus pruebas y su línea en el [CHANGELOG](../CHANGELOG.md).

Leído en: este repo en `cdbe5f3`; SoundBender (`soundshop`) `372055e`, en especial `docs/plans/profiles-and-controls.md`. Antes de empezar, la sesión del core **debe** releerlos (`git log`) por si cambiaron.

## 1. Lo que dijo el usuario (2026-11-27)

En sus palabras, resumidas:

- **Capas.** Hay cosas que afectan a todas las apps, con ajustes que cada app o cada perfil sobrescriben: control general y optimizaciones generales en el core, y cada app lo hace lo mejor y lo más nativo posible.
- **Rust para compartir con Windows,** sin duplicar lógica. Manda lo más eficiente, robusto y confiable.
- **Perfiles por app:** cada perfil de cada app guarda su biblioteca y su apariencia. Los equipos esperan al inner core y la nube, que traerán permisos.
- **Páginas dentro de una app** en el controlador, distintas de cambiar de app: en SoundBender, modo master, canal, efectos o mixer.
- **Stalker grabando las acciones de otra app** (SoundBender) para tratarlas en Clipper, como identificadores o disparadores, igual que los atajos que salen en pantalla con «qué pasó».

## 2. Regla general: capas y lenguaje (propuesta)

Para el [README](../README.md) o las [prácticas](practices.md), si el usuario la confirma como regla de la suite:

- **Capas de ajustes:** suite → app → perfil. El core define el valor de la suite; una app lo sobrescribe para sí; un perfil de la persona lo sobrescribe dentro de esa app. Cada capa escribe sólo lo suyo. Ya es así en `controllers/`: `suite.json` para lo de nadie, `<perfil>/<app>.json` para cada app ([surface §6](../contracts/nu9ve-surface-v1.md#6-mapas-por-aparato-app-modo-y-perfil)).
- **Lenguaje:** la lógica que también necesita Windows va en Rust, una sola vez. Lo propio del sistema (ventanas, controles, CoreMIDI o WinRT cuando no hay otra) va nativo en cada app. Si una pieza tiene versiones por lenguaje, comparten vectores.
- **Desempate:** lo más eficiente, robusto y confiable. Un archivo que escriben varias apps a la vez es el caso donde una sola implementación vale más que una versión nativa por lenguaje (§3).

## 3. Perfiles: una sola implementación

**Problema.** `profiles.json` lo escriben varias apps a la vez, cada una su clave, con `flock` y publicación atómica ([perfiles §4](../contracts/nu9ve-profile-v1.md#4-leer-y-escribir)). Hoy existe en Rust (`rust/nu9ve-profiles`, nove) y en Python (`scripts/nu9ve_profiles.py`). Una app de Mac en Swift (SoundBender) y las de Windows en C# no tienen cómo usarlo. Si cada lenguaje lo reimplementa, una diferencia en el bloqueo, la validación o la conservación de campos desconocidos rompe el archivo de otra app.

| Opción | Cómo | A favor | En contra |
| --- | --- | --- | --- |
| **A. Interfaz C de `nu9ve-profiles`** | Un crate `nu9ve-profiles-ffi` con unas pocas funciones (`read`, `for_app`, `upsert_app_entry`, `remove_app_entry`, `create`, liberar) que pasan JSON o estructuras C. XCFramework para `swift/` y DLL para `windows/`, con una capa fina `Nu9veProfiles` en Swift y `Nu9ve.Core.Profiles` en C# que sólo traducen tipos | La lógica queda una vez, para Mac y Windows, como pidió el usuario. Los ejemplos del contrato (`data/profile-examples-v1.json`) prueban las tres capas | El primer binario de Rust dentro de `swift/`: compilarlo para arm64 y x86_64, firmarlo dentro de cada app, y `sync-core.sh` debe copiar el artefacto o construirlo. SoundBender ya enlaza su propio XCFramework de Rust, así que el camino existe en una app |
| **B. Una versión por lenguaje con vectores** | `Nu9veProfiles` en Swift y su par en C#, probados contra los mismos ejemplos, como `Nu9veRecording` y `Nu9veMIDI` | Sin binarios; nativo en cada app; el precedente de hoy | Tres escritores del mismo archivo compartido; cada regla nueva del contrato se programa tres veces |

**Recomendación:** A, por la regla del §2 (un archivo compartido entre apps y sistemas). Si A pesa demasiado ahora, B con una condición: ninguna versión nativa escribe hasta que pase los mismos ejemplos de escritura concurrente que el crate. **Decide la sesión del core con el usuario.**

Lo que toma SoundBender, sea cual sea: perfil por omisión con su biblioteca de hoy en su sitio, apariencia por perfil, recuperación de la máquina, y `NU9VE_PROFILES_FILE` dentro de su raíz de QA.

## 4. Páginas dentro de una app (surface y mapas)

**Problema.** En [surface §5](../contracts/nu9ve-surface-v1.md#5-páginas-zonas-y-modos) una página es una **app**, y los modos son `normal` y `recording`. Una app con varias tareas para el mismo aparato (SoundBender: pads, canal, mixer, master, efectos) no tiene dónde ponerlas: el mapa sólo admite una asignación por control.

**Propuesta, versión menor de `nu9ve-controller-map-v1` §3 y `nu9ve-surface-v1` §4–§6:**

- **Mapa:** por aparato, `pages: [{"id": "mixer", "title": "Mixer", "mappings": […]}]`, opcional. Los `mappings` de arriba son los de todas las páginas; los de una página reemplazan los de esos controles mientras está activa, como hoy `modes.recording`. Orden de resolución: modo (`recording`) → página interna → mapeos comunes.
- **Cambiar de página interna:** una acción reservada del catálogo de cada app, `surface.page.next` y `surface.page.previous` (y `surface.page.<id>` para ir a una), que la persona mapea como cualquier otra. Así no hace falta un segundo control reservado por aparato, y el de cambiar de app (`pageControl`) no cambia. Alternativa a discutir: pulsación larga de `pageControl` = página interna.
- **Mensajes:** el agente lleva la página interna de cada app y avisa con `subpage {app, page}`; la app la muestra (su HUD, sus luces con `states`) y puede pedirla con `subpage` desde su interfaz. Al volver a la app, sigue en la última página interna.
- **Conflictos:** `SurfaceMaps` suma `page` (una página que mapea un control de zona de otra app) y `unknown_page` (un mapeo a una página que no existe).
- **Hasta que exista:** la app puede resolver sus páginas ella misma con una acción «Página siguiente» y mapeos propios; por eso C4 empieza sin esperar.

## 5. Eventos de acción para Stalker y Clipper

**Problema.** Stalker graba pantalla y audio; Clipper los edita. Si otra app de la suite dice qué acciones hizo la persona mientras tanto (con el teclado, un controlador o el menú), Clipper puede mostrar el globo de «qué pasó» como los atajos en pantalla, poner marcas y cortar por acción. Hoy no hay cómo: la [señal de grabación](../contracts/nu9ve-recording-v1.md) da el reloj (§5) y dice que lo que escribe un consumidor «llega a la sesión por su propio contrato», que no existe para acciones.

**Propuesta: `nu9ve-action-events-v1`**, hermano de `web.ndjson` del [navegador §7](suite-browser.md#7-contratos-nuevos):

- **Cuándo:** sólo mientras la señal está en `capturing`, y sólo si la persona lo activó en la app (apagado por omisión).
- **Qué, una línea NDJSON por acción:** `t` (µs, reloj de la sesión, grabación §5), `session_id`, `app`, `action` (id estable del catálogo de [interacción §1](../contracts/nu9ve-interaction-v1.md)), `title` (como lo ve la persona), `keys` si llegó por teclado, `value` o `delta` si es continua (una línea al soltar, no por mensaje), y `via`: `keyboard`, `controller`, `menu`, `pointer` o `automation`.
- **Dónde:** una carpeta por sesión que Stalker publica en su estado (por ejemplo `events/<app>.ndjson`), para que cada app escriba sólo su archivo; Stalker lo mete en el bundle al cerrar.
- **Privacidad, Debe:** nunca texto escrito, nombres de archivo ni contenido; sólo ids y títulos del catálogo. Un campo de texto con foco no emite nada.
- **Clipper:** lo lee como lee los eventos del navegador; el globo usa `title` y `keys`.

Sale casi gratis de un catálogo de acciones con ids estables, que ya pide [interacción §1](../contracts/nu9ve-interaction-v1.md) y que SoundBender va a escribir primero (es la C6 de [controles comunes](suite-ui.md)).

## 6. Orden propuesto

| Paso | Qué | Quién | Depende de |
| --- | --- | --- | --- |
| 1 | Decidir A o B para los perfiles (§3) y la regla del §2 | Usuario y sesión del core | — |
| 2 | Perfiles en Swift (y C#) según lo decidido, con los ejemplos del contrato | Core | 1 |
| 3 | Páginas internas en el mapa, el agente y `SurfaceMaps` (§4) | Core | C3 (Stalker cliente) |
| 4 | `nu9ve-action-events-v1` (§5), con Stalker como receptor y Clipper como lector | Core, Stalker y Clipper | Catálogo con ids en la primera app |

Mientras tanto, SoundBender hace su catálogo con ids estables, sus perfiles con lo que haya y lo básico del controlador resolviendo sus páginas ella misma.

## 7. Preguntas para el usuario

1. ¿La regla del §2 (capas suite → app → perfil; Rust para lo que comparte Windows; nativo lo demás) entra al README o a las prácticas?
2. Perfiles: ¿A (interfaz C de Rust, primer binario en `swift/`) o B (versión nativa por lenguaje con vectores)?
3. Páginas internas: ¿acción reservada en el catálogo de cada app, o pulsación larga del control de página?
4. Eventos de acción: ¿apagados por omisión y activados por app, o encendidos para todas mientras Stalker graba?
