# Uso de agentes por cuenta y vistas por categoría

Propuesta para la siguiente sesión del core, 2026-11-26; **decidida** el mismo día ([abajo](#decisión-2026-11-26)). Primer consumidor: nove ([A101](../../nove/docs/decisions.md#a101--uso-por-cuenta-y-mapas-de-calor-reutilizables-2026-11-26)). Los contratos vigentes de catálogo y perfiles conservan su significado.

## Evidencia de identidad y uso

- Mantener separados el proveedor, la cuenta histórica en ese proveedor, el dispositivo, el modelo y el perfil local de la suite. Una persona puede usar varias cuentas de un proveedor en una misma Mac. Una copia en otro disco conserva el dispositivo y la procedencia originales.
- Validar qué metadatos históricos de cuenta y de título aportan los formatos de cada proveedor o sus exportaciones. Usar una etiqueta local y una clave opaca cuando haya evidencia expresa. Permitir asociaciones declaradas por la persona a una fuente aislada o a sesiones concretas, dejando constancia de su procedencia. Si se asigna un rango temporal, conservarlo para que otras sesiones de la misma raíz no hereden la cuenta.
- Representar la atribución desconocida y los conflictos entre evidencias. Excluir los archivos de autenticación, tokens de acceso, claves API y perfiles de navegador del lector y de las copias. Mantener títulos, alias de cuentas y datos de uso como personales según [nu9ve-data-v1](../contracts/nu9ve-data-v1.md) y [nu9ve-profile-v1](../contracts/nu9ve-profile-v1.md).
- En métricas por turno distinguir caracteres por rol y tokens reportados (entrada, salida y caché), con la cobertura de cada métrica. Cotejar un turno con varios mensajes del asistente, metadatos duplicados, cambio de modelo, una transcripción incompleta y un contador acumulado que se reinicia. Usar valores nulos para las medidas que no se puedan atribuir. Una corrida de benchmark correlacionada por commit conserva ese tipo de evidencia.

## Vistas reutilizables

- Probar una codificación en la que el tono indique una categoría y la intensidad una medida, manteniendo la misma escala y leyenda entre categorías. La primera variante de nove aplica el tono a la fila del mapa de calor.
- Para una categoría independiente de los ejes, comparar paneles por categoría y segmentos dentro de cada casilla. Documentar cómo se representan varias categorías en la misma casilla y conservar el total. Limitar la cardinalidad e identificar «Otros»; conservar valores ausentes, unidad y número de observaciones. Ofrecer la variante sólo si el dataset tiene un campo categórico y una medida compatibles.
- Validar el diseño con sesiones en dos dispositivos y cuentas distintas y con una tabla ajena a conversaciones, por ejemplo benchmarks por motor y semana. Si una segunda app necesita el mismo formato o control y se confirma la semántica, proponer ejemplos y la pieza compartida antes de mover código.

La salida de la sesión del core es una decisión sobre qué semántica se comparte, con dos casos que la ejerciten. Los lectores, la publicación de métricas y la aceptación de la vista de nove se mantienen en su repo.

## Decisión (2026-11-26)

Tomada por el usuario en la sesión del core, después de leer los lectores de nove (`core/src/conversations/`, con su A101 sin commit) y la copia de nüün (`nuun-media/src/conversations.rs`).

**Dónde vive: sólo en nove por ahora.** nove es el único que interpreta conversaciones; nüün las copia sin leerlas (`004_datos/conversaciones/<colección>/<device_id>/…`, con el manifiesto `nuun-conversations-working/1`). No hay dos lectores, así que no hay contrato ni crate en el core: la forma la define nove, siguiendo esta decisión, y sube al core cuando aparezca un segundo lector. Lo que ya comparten las dos apps es el manifiesto de la copia; subirlo al core con un nombre de la suite es un paso aparte, que cambia nüün y nove a la vez.

**La cuenta sale de los archivos, sin que la persona declare nada.** Se revisaron los nombres de los campos, no sus valores, de las sesiones de la M1:

| Proveedor | Evidencia por sesión | En la M1 |
| --- | --- | --- |
| Claude Code | `ownerAccountUuid` y `ownerOrganizationUuid` en las líneas; `accountUuid`; `attachment.organizationUuid` | 252 de 261 sesiones traen la organización y 133 la cuenta dueña; 2 cuentas y 2 organizaciones distintas; ninguna sesión mezcla cuentas. Las 9 sin campos son de versiones viejas (2.1.281 y anteriores) |
| Codex | `creator_account_id` y `creator_user_id` en `session_meta` | Las 22 sesiones; una cuenta |
| Claude Desktop | La ruta `claude-code-sessions/<account>/<org>/` | 2 cuentas |

- El lector guarda una **clave opaca** por cuenta: un SHA-256 de `<proveedor>:<id>`, recortado. El identificador crudo no se guarda ni se publica. Es personal ([data §6](../contracts/nu9ve-data-v1.md#6-privacidad)).
- La persona **puede** ponerle nombre a cada cuenta una vez («personal», «trabajo»); nunca por proyecto ni por carpeta. Sin nombre, la vista dice «Cuenta 1», «Cuenta 2» por proveedor.
- Orden de la evidencia: el campo de la sesión, después la ruta de Desktop, después lo que la persona asocie. Sin ninguna, `unknown`. Si dos evidencias de una misma sesión dicen cuentas distintas, `conflict`, y la sesión no se suma a ninguna.
- No se lee ni se copia ningún archivo de autenticación (`~/.claude.json`, `.credentials.json`, `auth.json`): la cuenta de hoy no dice la de cada sesión. `account_link` de [perfiles](../contracts/nu9ve-profile-v1.md) queda para la cuenta de la suite, no la del proveedor.
- Con la clave por cuenta y el `device_id` que ya da la copia, las sesiones de un mismo proyecto trabajado con cuentas distintas en la M1 y la M4 se separan solas.

**Turnos y tokens.**

- **Turno nativo, declarado:** cada proveedor usa el suyo (el prompt humano en Claude Code, el `turn_id` en Codex) y el registro dice cuál en `turn_basis`. Nadie reinterpreta los datos de un proveedor.
- **Codex por turno:** la diferencia del acumulado `total_token_usage` contra el turno anterior. Si el acumulado baja (se reinició), ese turno queda nulo y la cobertura lo cuenta. Claude Code sigue sumando por mensaje sin repetir `message.id`, como hoy.
- **Entrada sin caché**, como hoy en los dos: `input_tokens` excluye lo leído de caché, que va en `cache_read_tokens`, y lo escrito en `cache_write_tokens`.
- **Nulo no es cero:** una medida que el proveedor no reportó queda nula, también dentro de una sesión con otras medidas.

**Los dos casos que la ejercen** (en las pruebas de nove):

1. Claude Code: una sesión con una respuesta repetida (mismo `message.id`), un cambio de modelo y dos cuentas en dos dispositivos para el mismo proyecto.
2. Codex: una sesión cuyo acumulado se reinicia a mitad, con el turno nulo y la cobertura que lo dice.

**Las vistas** siguen en nove ([data-views-v1](../../nove/docs/contracts/data-views-v1.md)): `color: row` (tono por fila e intensidad con una sola escala), hasta 20 filas más «Otros», con `n` y unidad. Se proponen al core cuando otra app pida el mismo control.
