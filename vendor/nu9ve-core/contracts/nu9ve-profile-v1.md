# Perfiles en esta máquina v1

Estado: **propuesta** (2026-11-25). Es la pieza local de los perfiles del [modelo de la suite](../docs/suite-devices.md#5-modelo): un registro por máquina de los perfiles (las identidades de trabajo, como `e64` en nüün) y, por cada app, dónde vive la biblioteca de ese perfil. Así nüün sabe qué perfiles existen en la computadora y en qué apps, aunque un perfil exista sólo en una, y nove abre la biblioteca de otro perfil sin adivinar rutas. Nace del pedido A98 de nove. Ejemplos: [`data/profile-examples-v1.json`](../data/profile-examples-v1.json). Implementaciones: el crate `rust/nu9ve-profiles` y el ayudante [`scripts/nu9ve_profiles.py`](../scripts/nu9ve_profiles.py), que es la referencia.

**Debe** es obligatorio. **Conviene** es lo recomendado. «Archivo» es `profiles.json`. «Perfil» es una identidad de trabajo de la persona en esta máquina. «App» es el nombre corto de [`nu9ve-registry-v1` §2](nu9ve-registry-v1.md#2-archivo) (`nove`, `nuun`).

## 0. Para qué y para qué no

- **Para** decir, en esta máquina, qué perfiles hay, de qué tipo, y en qué apps tienen biblioteca y dónde. Una app lo lee para ofrecer cambiar de perfil o mostrar «otros perfiles en esta Mac»; nüün lo lee para saber en qué apps trabaja cada perfil.
- **No** es una cuenta ni una credencial: no da acceso a nada y no paga nada. El vínculo con la cuenta de Fractos (F2) y el espacio (§6.13 de [suite-devices](../docs/suite-devices.md#613-espacios-lo-personal-y-los-equipos)) son campos opcionales que hoy nadie llena.
- **No** guarda datos de los perfiles: sólo nombres y rutas. Cada biblioteca sigue siendo de su app, con su propio formato.
- **No** sale de la máquina: las rutas son personales (§7).

## 1. Dónde

| Sistema | Archivo |
| --- | --- |
| macOS | `~/Library/Application Support/nu9ve/profiles.json` |
| Windows | `%LOCALAPPDATA%\nu9ve\profiles.json` |
| Linux | `$XDG_DATA_HOME/nu9ve/profiles.json` (`~/.local/share` si falta la variable). Sólo para pruebas |

- Junto a [`device.json`](nu9ve-device-v1.md#1-dónde), en la carpeta padre de la del [registro de motores](nu9ve-registry-v1.md#1-dónde).
- **Debe:** la variable `NU9VE_PROFILES_FILE` (ruta absoluta) reemplaza el archivo. Las pruebas y la QA la usan **siempre**: un proceso con raíz de QA no lee ni escribe el archivo de la persona. Si la variable no es absoluta, no hay archivo: no se adivina otra ruta.
- **Debe** con sandbox o paquete: como [`device.json`](nu9ve-device-v1.md#1-dónde), un proceso que no alcanza la carpeta real no crea otro archivo en su contenedor; trabaja sin perfiles y lo dice. En Windows la carpeta se declara sin virtualizar.

## 2. Archivo

```json
{
  "contract": "nu9ve-profile/1",
  "device_id": "6f0c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f",
  "profiles": [
    {
      "profile_id": "e64",
      "display_name": "Erik",
      "kind": "person",
      "aliases": ["erikiado64"],
      "default_space": null,
      "account_link": null,
      "created_at": "2026-11-25T18:00:00Z",
      "apps": {
        "nuun": {"library": "/Users/ana/Library/Application Support/nuun/profiles/e64", "last_used_at": "2026-11-25T18:00:00Z", "default": true, "ui": null},
        "nove": {"library": "/Users/ana/Library/Application Support/nove-development/engine", "last_used_at": "2026-11-25T19:30:00Z", "default": true, "ui": "full"}
      }
    }
  ],
  "updated_at": "2026-11-25T19:30:00Z"
}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `contract` | Sí | `"nu9ve-profile/1"` |
| `device_id` | Sí | El de [`device.json`](nu9ve-device-v1.md#2-archivo) cuando el escritor lo conoce, o `null`. Sólo dice en qué máquina se escribió; un lector **no** lo compara para decidir nada |
| `profiles` | Sí | Lista de perfiles, como mucho **64** |
| `updated_at` | Sí | RFC 3339 en UTC con `Z`: el último cambio |
| `retired` | No | Ids de perfiles borrados (§5), como mucho 1024. Nunca se reutilizan |

Cada perfil:

| Campo | Debe | Qué es |
| --- | --- | --- |
| `profile_id` | Sí | `^[a-z0-9][a-z0-9-]{0,31}$`. Estable: **nunca cambia** y no se reutiliza para otra persona. El de nüün (`e64`) es el mismo aquí |
| `display_name` | Sí | Lo que lee la persona: 1 a 64 caracteres, sin caracteres de control |
| `kind` | Sí | `person` (una persona), `team` (trabajo de un equipo) o `viewer` («ver como»: una vista de cómo verá la app otra persona, como el perfil del padre en Windows) |
| `aliases` | No | Otros nombres (`erikiado64`): hasta 16, sin repetir, con la regla de `display_name`. Falta = `[]` |
| `default_space` | No | Espacio por omisión ([§6.13](../docs/suite-devices.md#613-espacios-lo-personal-y-los-equipos)): cadena de 1 a 128 o `null`. Hoy siempre `null` |
| `account_link` | No | Vínculo con la cuenta de Fractos (F2): cadena de 1 a 256 o `null`. Hoy siempre `null` |
| `created_at` | Sí | RFC 3339 en UTC con `Z` |
| `apps` | Sí | Objeto `{"<app>": entrada}`, como mucho **32** apps. Puede ir vacío: un perfil puede existir sin biblioteca en ninguna app |

Cada entrada de app:

| Campo | Debe | Qué es |
| --- | --- | --- |
| `library` | Sí | Ruta **absoluta** de la biblioteca de ese perfil en esa app (POSIX `/…`, `C:\…` o UNC `\\…`), hasta 1024 caracteres, sin caracteres de control |
| `default` | Sí | `true` en el perfil que la app abre si no se le dice otro. **Como mucho uno** por app en todo el archivo |
| `last_used_at` | No | Cuándo la app abrió por última vez ese perfil, o `null` |
| `ui` | No | `full`, `simple` o `null`: la vista que la app usa con ese perfil (nove: `simple` es la de Windows del padre). Cada app decide qué significa |

- **Debe:** UTF-8 sin BOM, un objeto JSON, como mucho **64 KiB**.
- **Debe** al lector: ignorar campos que no conoce, en el documento, en un perfil o en una entrada; y el **escritor** los conserva al reescribir.
- **No debe:** secretos, tokens ni contenido de las bibliotecas.

## 3. Validar

Un archivo es válido si cumple §2. El primer problema, en este orden, da su código (los mismos en Python y Rust; los ejemplos los fijan):

`too_large`, `bom`, `not_json`, `not_object`, campos del documento (`missing_field`, `wrong_type`), `unknown_contract`, `bad_device_id`, `bad_time`, `too_many_profiles`; por cada perfil en orden: `wrong_type`/`missing_field`, `bad_profile_id`, `bad_name`, `bad_kind`, `bad_time`, `bad_alias`, `bad_space`, `bad_account_link`, `too_many_apps` y, por app en orden de código, `bad_app`, `wrong_type`/`missing_field`, `bad_library`, `bad_time`, `bad_ui`; después `duplicate_profile` (al encontrar el repetido), `two_defaults` y `bad_retired` (un id inválido, repetido o que sigue en uso).

- **Debe** en macOS y Linux: un archivo que no es regular (un enlace) o que el grupo u otros pueden leer es inválido (`unsafe_permissions`).
- **Debe:** un archivo inválido se deja **como está**: nadie lo borra ni lo reemplaza. Un lector trabaja como si no hubiera perfiles y lo dice; un escritor se niega (`invalid`).

## 4. Leer y escribir

- **Leer** no toma candado: la escritura es atómica. Se lee con tope (64 KiB).
- **Escribir** cambia el archivo entero, siempre así:
  1. crea la carpeta `nu9ve/` si falta, `0700` en macOS y Linux;
  2. toma el candado **exclusivo** de `profiles.json.lock`, hermano del archivo (se crea `0600` y nunca se borra): `flock(LOCK_EX)` en macOS y Linux, `LockFileEx` en Windows. Espera como mucho 5 s; si no lo obtiene, no escribe (`busy`);
  3. lee el archivo; si no existe, parte de `{"contract", "device_id", "profiles": [], "updated_at"}`; si es inválido, se detiene (`invalid`);
  4. aplica **un** cambio de §5 y comprueba el resultado con §3;
  5. escribe un temporal hermano `0600`, lo sincroniza y lo renombra sobre `profiles.json` ([`nu9ve-fs`](nu9ve-fs-v1.md)); suelta el candado.
- **Debe:** si el archivo no tiene `device_id` y el escritor lo conoce, lo pone. Nunca cambia uno que ya está.
- **Debe:** quien escribe sin el candado rompe a los demás. El crate y el ayudante de Python toman el mismo; una app en Swift o C# usa la misma llamada del sistema sobre el mismo archivo.

## 5. Cambios

Un cambio es un objeto JSON con `op` y `profile_id`. Lo que se niega no escribe nada y da su código.

| `op` | Campos | Hace | Se niega con |
| --- | --- | --- | --- |
| `create_profile` | `display_name`, `kind`, y opcionales `aliases`, `default_space`, `account_link` | Agrega un perfil con `apps: {}` y `created_at` = ahora. **Sólo agrega** | `bad_change` (falta `display_name` o `kind`), `bad_name`, `bad_kind`, `bad_alias`, `bad_space`, `bad_account_link`, `profile_exists`, `profile_retired`, `too_many_profiles` |
| `update_profile` | Los mismos, todos opcionales | Cambia sólo los campos que trae. `profile_id` y `created_at` no cambian | Los mismos de validación, `unknown_profile` |
| `upsert_app_entry` | `app`, `library`, `default` (falta = `false`), `ui` (falta = `null`) | Crea o reemplaza la entrada de **esa** app, con `last_used_at` = ahora, conservando sus campos desconocidos. Con `default: true`, las entradas de esa misma app en los demás perfiles pasan a `false` | `bad_app`, `unknown_profile`, `bad_library`, `bad_change` (`default` no es booleano), `bad_ui`, `too_many_apps` |
| `remove_app_entry` | `app` | Quita la entrada de esa app (si no está, no hace nada) | `bad_app`, `unknown_profile` |
| `delete_profile` | — | Quita el perfil y agrega su id a `retired` | `unknown_profile`, `profile_in_use` (alguna app tiene entrada) |

Todos: `bad_profile_id` primero si el id no cumple §2; `bad_change` si `op` es otro; y, después del cambio, cualquier código de §3 (como `too_large`).

Reglas:

- **Debe:** cada app escribe **sólo su clave** en `apps` (`upsert_app_entry` y `remove_app_entry` con su propio `app`). Crear o renombrar un perfil lo puede hacer cualquier app, con la confirmación de la persona.
- **Debe:** crear un perfil sólo agrega. Si el id ya existe (porque otra app lo creó), la app agrega su entrada a ese perfil, no crea otro.
- **Debe:** borrar un perfil exige que ninguna app lo use: cada app quita antes su entrada, y quitarla **no** borra la biblioteca.
- **Debe:** un `profile_id` no se reutiliza para otra persona: queda en `retired`.

## 6. Listar por app

`for_app(app)` da los perfiles con entrada para esa app, en este orden: el `default` primero, luego por `last_used_at` más reciente (sin fecha al final), luego por `profile_id`. Lo que una app muestra en su selector de perfiles.

## 7. Privacidad

- Las rutas son **personales**: dicen el nombre del usuario y dónde guarda su trabajo. El archivo **no** sale de la máquina, no va en registros de errores, telemetría ni [benchmarks](nu9ve-bench-v1.md), y una app no manda la ruta de la biblioteca de **otro** perfil a un agente ni por MCP.
- Una app que muestra otros perfiles abre sus bibliotecas **en sólo lectura** y enseña conteos y fechas, no contenidos.
- `profile_id` y `display_name` no son secretos: el [registro de motores](nu9ve-registry-v1.md#2-archivo) puede decir el perfil de un motor vivo.

## 8. Ejemplos

[`data/profile-examples-v1.json`](../data/profile-examples-v1.json): dónde está el archivo según el sistema y las variables, archivos válidos e inválidos con su código, secuencias de cambios con el documento final y los rechazos, y listados por app. `python3 -I scripts/profile-examples.py --check` los comprueba contra la referencia; `cargo test -p nu9ve-profiles` los corre en Rust, con escritores concurrentes.

## 9. Adopción

| App | Qué hace | Estado |
| --- | --- | --- |
| nove | Primer escritor: su biblioteca personal es el perfil `e64`; un perfil nuevo es una biblioteca nueva; su motor lee el archivo (`profiles_overview`) y dice su perfil en el registro | En integración (su A98) |
| nüün | Lee el archivo para saber qué perfiles hay y en qué apps; agrega su entrada a sus perfiles (`e64`) | Pendiente |
| Atlas, Clipper | Leen con el ayudante de Python o el crate | Pendiente |

## 10. Decisiones abiertas

1. **Windows sin probar.** `LockFileEx` y la carpeta sin virtualizar están escritos, no probados en un paquete real. Lo prueba nove en Windows.
2. **Perfiles entre máquinas.** El mismo `profile_id` en dos máquinas es el mismo perfil, pero este archivo es local. Unirlos llega con la cuenta (F2) o con la carpeta de intercambio.
3. **Quién borra.** Hoy ninguna app ofrece borrar un perfil; sólo dejarlo sin biblioteca.
