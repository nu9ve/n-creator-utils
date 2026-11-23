# Licencias: qué se abre y qué queda privado

Estado: **propuesta** (2026-11-17), pedida por el usuario desde una sesión de SoundBender. **Nada está decidido:** se afina a medida que se defina el negocio de cada app. Aquí no hay asesoría legal: antes de vender o de publicar un repo conviene una revisión con alguien de propiedad intelectual. [Mapa de la suite](suite.md) · [Cómo se comparte](suite-sharing.md) · [Audio §5](platforms/audio.md#5-curado-y-soundbender-pieza-por-pieza).

## 1. Cómo está hoy

- **Todos los repos de la suite son privados**, empezando por `nu9ve/core`.
- **Ninguno tiene un archivo `LICENSE` propio** (revisado el 2026-11-17: core, SoundBender, curado, nove, kyu, Clipper, neuf y Stalker). Sin licencia, el código es «todos los derechos reservados», aunque un día el repo se haga público. Abrir algo es una decisión explícita, repo por repo.
- **El `deny.toml` de `rust/`** admite solo licencias permisivas (MIT, Apache-2.0, BSD, ISC, Unicode y Zlib): nada copyleft, porque alguna app puede venderse como binario cerrado. curado depende de symphonia (MPL-2.0) y queda fuera de esa regla ([audio §5](platforms/audio.md#licencia-de-la-decodificación)).

## 2. Qué pide cada licencia de terceros

Lo que importa es qué obliga a hacer con **tu** código cuando distribuyes una app que la usa.

| Licencia | Al distribuir | ¿Permite vender cerrado? |
| --- | --- | --- |
| MIT, Apache-2.0, BSD, ISC, Zlib | Incluir el aviso de copyright y la licencia | Sí |
| MPL-2.0 | Publicar solo los archivos de esa dependencia que hayas modificado, e indicar dónde está su fuente. Tu código queda con su licencia | Sí |
| LGPL | Que el usuario pueda reemplazar la biblioteca (enlace dinámico en la práctica) | Sí, con cuidado al enlazar |
| GPL | La app entera pasa a ser GPL, con su fuente | No, salvo con licencia comercial del autor |

Casos de la suite ya conocidos:

- **symphonia** (MPL-2.0), decodificación de curado. Compatible con vender, si no se modifican sus archivos.
- **Bindings de VST3:** el SDK de Steinberg es MIT desde la 3.8, pero `vst3-sys` (el que usaba nih-plug) es GPLv3. El plugin de SoundBender tiene que elegir unos bindings con licencia que permita vender.
- **Ableton Link:** GPLv2 o comercial. Para una app cerrada, se compra la licencia o no se usa.

**Propuesta:** el `deny.toml` sigue sin admitir copyleft en general. MPL-2.0 se admite **solo como excepción por crate** (`[[licenses.exceptions]]`, empezando por `symphonia*`). GPL y AGPL nunca. Cada app con binario incluye la lista de licencias de terceros en Acerca de, generada desde `Cargo.lock` y los paquetes de Swift y C#.

## 3. Tres capas

### Público: `nu9ve/core`, cuando se decida abrirlo

Lo que es infraestructura y no diferencia a ninguna app:

- contratos, tema, interacción, sonido de interfaz y protocolo del motor;
- `nu9ve-json`, `nu9ve-fs`, `nu9ve-engine`;
- `NuMacShell`, `Nu9veTheme`, `Nu9ve.Core` y `Nu9ve.WinUI`;
- las capas de audio genéricas: `nu9ve-loudness` (LUFS y true peak) y, después, filtros y E/S de audio.

Licencia propuesta: **MIT o Apache-2.0, a elección de quien lo use**, como el ecosistema de Rust. Apache-2.0 añade una concesión de patentes. Abrir esto cuesta poco, porque no es lo que se vende, y da confianza y posibles contribuciones.

### Privado: un «inner core» (nombre por decidir)

Lo compartido que sí es negocio o toca datos de personas:

- cuentas, licencias y activación;
- sincronización y servicios en la nube;
- telemetría, cobros y actualizaciones con canal privado;
- conectores con servicios propios o de clientes.

Se consume igual que `nu9ve/core`: copia fijada en `vendor/` con su propia lista de rutas y el mismo script. Mientras todo sea privado, puede vivir en `nu9ve/core`, en carpetas marcadas, y separarse cuando el core se abra. Su primer contenido concreto (cliente de la nube, cuenta y dispositivos) está propuesto en [varias máquinas, discos y nube §9](suite-devices.md#9-público-y-privado-qué-va-dónde), que recomienda un repo propio desde el principio en lugar de carpetas marcadas, para no tener que limpiar la historia al abrir el core.

### Cada app

El motor de dominio y la interfaz de cada app son su producto: el editor espectral de SoundBender, los decks de curado y el motor editorial de Clipper. Cada app elige su modelo (§4).

## 4. Modelos para una app

| Modelo | Cómo es | Cuándo conviene |
| --- | --- | --- |
| **Open core** | Capas públicas en `nu9ve/core`; motor de dominio, interfaz y conectores privados; se venden los binarios | Para vender sin regalar el producto. **Propuesto para SoundBender** |
| **App abierta, binarios de pago** | Fuente pública (GPL o MIT); se cobra por el binario firmado, el soporte o la comodidad, como hace Ardour | Si importa la comunidad más que el ingreso. Con MIT, otro puede compilarla y venderla con otro nombre; con GPL, nadie puede cerrarla, pero el autor sí puede vender |
| **Código visible, no open source** | BSL, FSL o PolyForm: se puede leer, el uso comercial queda restringido; algunas pasan a abiertas tras unos años | Transparencia sin regalar el uso comercial |

## 5. Reglas que hacen que funcione

- **Un repo público no puede llevar copia de nada privado.** Una app que fija el inner core tiene que ser privada, o recibir esa parte ya compilada.
- **Un binario puede incluir partes privadas** aunque el resto del código sea público, siempre que lo público sea MIT, Apache, BSD o MPL. Se compila en un CI privado, con lo privado como módulo o feature que la build pública no trae. **Con GPL no se puede:** el binario combinado tendría que ser GPL entero.
- **Lo que protege un producto está en el servidor:** cuenta, nube y sincronización. Un chequeo de licencia dentro de un cliente abierto se quita en minutos.
- **Firmar y distribuir** es aparte de la licencia: Developer ID y notarización en Mac, y un certificado de firma en Windows ([decisión abierta sobre un certificado común](windows-shared-next.md)).

## 6. Antes de hacer público `nu9ve/core`

1. Quitar o mover lo que describe por dentro a apps privadas: inventarios de nüün, Clipper y curado, rutas `~/code/ai/...`, y detalles de nove (datos de mercado, Hermosillo) en [suite.md](suite.md), [suite-adoption.md](suite-adoption.md) y [adopción](adoption.md). Esos documentos pueden quedar en el inner core.
2. Buscar secretos y datos personales en toda la historia, no solo en `HEAD`.
3. Elegir la licencia y añadir `LICENSE` (y `NOTICE` si es Apache-2.0).
4. Decidir si se aceptan contribuciones externas y con qué acuerdo (DCO o CLA). Con un CLA, el autor puede seguir vendiendo licencias comerciales de lo que se abra.
5. Revisar que cada crate, paquete y script tenga una licencia compatible con la elegida.

## 7. Preguntas para el usuario

Ninguna urge. Se responden cuando el negocio de cada app esté más claro.

1. ¿Se abre `nu9ve/core` algún día? Si sí, ¿con MIT, Apache-2.0 o las dos?
2. ¿El inner core es un repo propio o, mientras todo sea privado, carpetas marcadas dentro de `nu9ve/core`? ¿Cómo se llama?
3. ¿Qué modelo sigue cada app? Para SoundBender la propuesta es open core.
4. ¿Se admite MPL-2.0 como excepción por crate (symphonia), o la decodificación queda en cada app con la API del sistema?
