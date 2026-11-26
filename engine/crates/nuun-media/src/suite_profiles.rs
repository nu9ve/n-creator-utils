//! Perfiles de la suite en esta máquina (nu9ve-profile-v1, `profiles.json`).
//!
//! - `suite_profiles` (lectura): qué perfiles hay, de qué tipo, en qué apps y dónde viven sus bibliotecas, con
//!   los perfiles creativos de nüün y si ya tienen su entrada `nuun`. Lo de otras apps es sólo lectura.
//! - `suite_profiles_register` (escritura, con la confirmación de la persona): agrega la entrada `nuun` a cada
//!   perfil creativo del catálogo. Si el perfil ya existe (nove lo crea primero con `e64`) sólo agrega su entrada
//!   y, si hace falta, el nombre de nüün como alias; nunca toca la entrada de otra app.
//!
//! Bajo QA (una raíz aislada) sólo se usa `NU9VE_PROFILES_FILE`: nunca el archivo de la persona (perfil §1).
//! Un archivo inválido se deja como está: la lectura lo dice y la escritura se niega (perfil §3).
use crate::catalog::Catalog;
use nu9ve_json::{Error, Result, require};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

pub const APP: &str = "nuun";
const ENV: &str = "NU9VE_PROFILES_FILE";

/// Dónde está `profiles.json` para este proceso, o `None` (QA sin la variable, o una variable no absoluta).
pub fn file(isolated: bool) -> Option<PathBuf> {
    if isolated && std::env::var_os(ENV).is_none() {
        return None;
    }
    nu9ve_profiles::default_file()
}

/// El documento, o el estado que explica por qué no hay ninguno.
pub fn load(path: Option<&Path>) -> (Option<Value>, Value) {
    let Some(path) = path else {
        return (
            None,
            json!({"state": "unavailable", "message": "Sin archivo de perfiles en esta raíz (QA sin NU9VE_PROFILES_FILE)"}),
        );
    };
    match nu9ve_profiles::read(path) {
        Ok(Some(doc)) => (Some(doc), json!({"state": "ok", "path": path})),
        Ok(None) => (None, json!({"state": "missing", "path": path})),
        Err(nu9ve_profiles::Error::Invalid(problem)) => (
            None,
            json!({"state": "invalid", "path": path, "problem": problem,
                   "message": "profiles.json no es válido; se deja como está"}),
        ),
        Err(e) => (
            None,
            json!({"state": "error", "path": path, "problem": e.code(), "message": e.to_string()}),
        ),
    }
}

/// Los perfiles creativos del catálogo de nüün y el de omisión (`defaults.profile`).
fn creative(catalog: &Catalog) -> Result<(Vec<Value>, Option<String>)> {
    let mut list: Vec<Value> = catalog
        .get("creative-profiles")?
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
        .into_values()
        .collect();
    list.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let default = catalog
        .get("defaults")?
        .and_then(|d| d["profile"].as_str().map(str::to_owned))
        .or_else(|| Some("e64".to_owned()));
    Ok((list, default))
}

/// La carpeta de una biblioteca existe (sólo `lstat`; nunca se abre su contenido).
fn present(path: &str) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir())
}

/// `suite_profiles`: lectura. `home` es la biblioteca de nüün (el catálogo); `isolated` sale de la raíz.
pub fn overview(home: &Path, isolated: bool) -> Result<Value> {
    overview_at(home, file(isolated).as_deref())
}

pub fn overview_at(home: &Path, path: Option<&Path>) -> Result<Value> {
    let (doc, state) = load(path);
    let (nuun, default) = creative(&Catalog::open(home)?)?;
    let mut profiles = Vec::new();
    for profile in doc
        .as_ref()
        .and_then(|d| d["profiles"].as_array())
        .into_iter()
        .flatten()
    {
        let mut apps: Vec<Value> = profile["apps"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(app, entry)| {
                let library = entry["library"].as_str().unwrap_or_default();
                json!({"app": app, "library": library, "default": entry["default"],
                       "last_used_at": entry["last_used_at"], "ui": entry["ui"],
                       "own": app == APP, "library_present": present(library)})
            })
            .collect();
        apps.sort_by(|a, b| a["app"].as_str().cmp(&b["app"].as_str()));
        profiles.push(json!({
            "profile_id": profile["profile_id"], "display_name": profile["display_name"],
            "kind": profile["kind"], "aliases": profile.get("aliases").cloned().unwrap_or(json!([])),
            "apps": apps}));
    }
    let nuun: Vec<Value> = nuun
        .iter()
        .map(|p| {
            let id = p["id"].as_str().unwrap_or_default();
            let found = doc.as_ref().and_then(|d| nu9ve_profiles::find(d, id));
            json!({"id": id, "name": p["name"], "default": default.as_deref() == Some(id),
                   "suite_id_ok": nu9ve_profiles::profile_id_ok(id),
                   "in_file": found.is_some(),
                   "registered": found.is_some_and(|f| f["apps"].get(APP).is_some())})
        })
        .collect();
    Ok(
        json!({"contract": "nuun-profiles/1", "file": state, "app": APP, "library": home,
              "profiles": profiles, "nuun": nuun}),
    )
}

/// Las bibliotecas que el archivo declara, como `(perfil, nombre, app, ruta)`: datos vivos, nunca acumulados.
pub fn libraries(doc: Option<&Value>) -> Vec<(String, String, String, PathBuf)> {
    let mut out = Vec::new();
    for profile in doc
        .and_then(|d| d["profiles"].as_array())
        .into_iter()
        .flatten()
    {
        let (Some(id), Some(name)) = (
            profile["profile_id"].as_str(),
            profile["display_name"].as_str(),
        ) else {
            continue;
        };
        for (app, entry) in profile["apps"].as_object().into_iter().flatten() {
            if let Some(library) = entry["library"].as_str() {
                out.push((
                    id.to_owned(),
                    name.to_owned(),
                    app.clone(),
                    PathBuf::from(library),
                ));
            }
        }
    }
    out
}

fn refused(e: nu9ve_profiles::Error) -> Error {
    match e {
        nu9ve_profiles::Error::Invalid(problem) => Error::new(
            "profiles_invalid",
            format!("profiles.json no es válido ({problem}); se deja como está"),
        ),
        nu9ve_profiles::Error::Busy => {
            Error::new("busy", "profiles.json está bloqueado por otro proceso")
        }
        other => Error::new("profiles_refused", other.to_string()),
    }
}

/// `suite_profiles_register`: agrega la entrada `nuun` a los perfiles creativos elegidos (todos si falta
/// `profiles`). `kinds` da el tipo de un perfil que aún no existe; por omisión `person` para el de omisión y
/// `team` para los demás. Pide `confirm: true`: crear o renombrar un perfil compartido es una decisión.
pub fn register(
    home: &Path,
    isolated: bool,
    device_id: Option<&str>,
    params: &Value,
) -> Result<Value> {
    let path = file(isolated).ok_or_else(|| {
        Error::new(
            "profiles_unavailable",
            "Sin archivo de perfiles en esta raíz (QA sin NU9VE_PROFILES_FILE)",
        )
    })?;
    register_at(home, &path, device_id, params)
}

pub fn register_at(
    home: &Path,
    path: &Path,
    device_id: Option<&str>,
    params: &Value,
) -> Result<Value> {
    let given = match params {
        Value::Null => Map::new(),
        Value::Object(o) => o.clone(),
        _ => return Err(Error::new("invalid_params", "params must be an object")),
    };
    require(
        given
            .keys()
            .all(|k| ["profiles", "kinds", "confirm"].contains(&k.as_str())),
        "invalid_params",
        "Only profiles, kinds and confirm are accepted",
    )?;
    require(
        given.get("confirm") == Some(&json!(true)),
        "confirmation_required",
        "Confirma que nüün agregue sus perfiles al archivo de perfiles de esta Mac",
    )?;
    let chosen: Option<Vec<String>> = match given.get("profiles") {
        None => None,
        Some(Value::Array(list)) => Some(
            list.iter()
                .map(|v| v.as_str().map(str::to_owned))
                .collect::<Option<_>>()
                .ok_or_else(|| Error::new("invalid_params", "profiles is a list of ids"))?,
        ),
        Some(_) => return Err(Error::new("invalid_params", "profiles is a list of ids")),
    };
    let kinds = given.get("kinds").cloned().unwrap_or(json!({}));
    require(
        kinds.is_object(),
        "invalid_params",
        "kinds maps ids to person, team or viewer",
    )?;
    let (nuun, default) = creative(&Catalog::open(home)?)?;
    let library = home
        .to_str()
        .filter(|l| nu9ve_profiles::library_ok(l))
        .ok_or_else(|| {
            Error::new(
                "invalid_library",
                "La biblioteca de nüün no es una ruta absoluta válida",
            )
        })?
        .to_owned();
    // Un archivo inválido se niega antes de cambiar nada.
    let _ = nu9ve_profiles::read(path).map_err(refused)?;
    let mut done = Vec::new();
    for profile in &nuun {
        let id = profile["id"].as_str().unwrap_or_default();
        if chosen.as_ref().is_some_and(|c| !c.iter().any(|x| x == id)) {
            continue;
        }
        if !nu9ve_profiles::profile_id_ok(id) {
            done.push(json!({"id": id, "result": "skipped", "reason": "bad_profile_id"}));
            continue;
        }
        let name = profile["name"].as_str().unwrap_or(id);
        let is_default = default.as_deref() == Some(id);
        let doc = nu9ve_profiles::read(path).map_err(refused)?;
        let existing = doc
            .as_ref()
            .and_then(|d| nu9ve_profiles::find(d, id))
            .cloned();
        let mut steps = Vec::new();
        match &existing {
            None => {
                let kind = kinds[id]
                    .as_str()
                    .unwrap_or(if is_default { "person" } else { "team" });
                let display: String = name.chars().take(64).collect();
                let change = json!({"op": "create_profile", "profile_id": id,
                                    "display_name": display, "kind": kind});
                nu9ve_profiles::change(path, &change, device_id).map_err(refused)?;
                steps.push("created");
            }
            Some(found) => {
                let mut aliases: Vec<Value> =
                    found["aliases"].as_array().cloned().unwrap_or_default();
                let known = found["display_name"] == name || aliases.iter().any(|a| a == name);
                if !known
                    && nu9ve_profiles::name_ok(name)
                    && aliases.len() < nu9ve_profiles::MAX_ALIASES
                {
                    aliases.push(json!(name));
                    let change =
                        json!({"op": "update_profile", "profile_id": id, "aliases": aliases});
                    nu9ve_profiles::change(path, &change, device_id).map_err(refused)?;
                    steps.push("alias");
                }
            }
        }
        let entry = nu9ve_profiles::upsert_app_change(id, APP, &library, is_default, None);
        nu9ve_profiles::change(path, &entry, device_id).map_err(refused)?;
        steps.push("entry");
        done.push(json!({"id": id, "result": "registered", "steps": steps}));
    }
    let mut answer = overview_at(home, Some(path))?;
    answer["registered"] = json!(done);
    Ok(answer)
}

/// El perfil de omisión de nüün si ya tiene su entrada `nuun` (para `profile_id` del registro de motores).
pub fn registered_default(home: &Path, isolated: bool) -> Option<String> {
    let path = file(isolated)?;
    let doc = nu9ve_profiles::read(&path).ok()??;
    let (_, default) = creative(&Catalog::open(home).ok()?).ok()?;
    let id = default?;
    nu9ve_profiles::find(&doc, &id)?["apps"].get(APP)?;
    Some(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn catalog(home: &Path) {
        crate::catalog::fixture::create(home);
        crate::catalog::WritableCatalog::open(home)
            .unwrap()
            .set(
                "creative-profiles",
                &json!({
                    "e64": {"id": "e64", "name": "erikiado64", "raw_dir": "e64_raw"},
                    "nu9ve-xyz": {"id": "nu9ve-xyz", "name": "nu9ve.xyz", "raw_dir": "nu9ve-xyz"}}),
            )
            .unwrap();
        crate::catalog::WritableCatalog::open(home)
            .unwrap()
            .set("defaults", &json!({"profile": "e64"}))
            .unwrap();
    }

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap().join("media");
        std::fs::create_dir_all(&home).unwrap();
        catalog(&home);
        let file = home.parent().unwrap().join("nu9ve/profiles.json");
        (dir, home, file)
    }

    #[test]
    fn a_missing_file_reads_as_none_and_registering_creates_it() {
        let (_dir, home, file) = setup();
        let read = overview_at(&home, Some(&file)).unwrap();
        assert_eq!(read["file"]["state"], "missing");
        assert_eq!(read["nuun"][0]["registered"], false);
        assert_eq!(
            register_at(&home, &file, None, &json!({}))
                .unwrap_err()
                .code,
            "confirmation_required"
        );
        let done = register_at(
            &home,
            &file,
            Some("6f0c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f"),
            &json!({"confirm": true}),
        )
        .unwrap();
        assert_eq!(done["file"]["state"], "ok");
        let doc = nu9ve_profiles::read(&file).unwrap().unwrap();
        let e64 = nu9ve_profiles::find(&doc, "e64").unwrap();
        assert_eq!(e64["kind"], "person");
        assert_eq!(e64["apps"]["nuun"]["default"], true);
        assert_eq!(
            nu9ve_profiles::find(&doc, "nu9ve-xyz").unwrap()["kind"],
            "team"
        );
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // Again: nothing new, still valid.
        register_at(&home, &file, None, &json!({"confirm": true})).unwrap();
        assert!(
            nu9ve_profiles::document_problem(&nu9ve_profiles::read(&file).unwrap().unwrap())
                .is_none()
        );
    }

    #[test]
    fn a_file_from_nove_keeps_its_entry_and_only_gains_nuun() {
        let (_dir, home, file) = setup();
        let nove = nu9ve_profiles::create_profile_change("e64", "Erik", "person");
        nu9ve_profiles::change(&file, &nove, None).unwrap();
        let entry = nu9ve_profiles::upsert_app_change(
            "e64",
            "nove",
            "/Users/ana/nove/engine",
            true,
            Some("full"),
        );
        nu9ve_profiles::change(&file, &entry, None).unwrap();
        let before = nu9ve_profiles::read(&file).unwrap().unwrap();
        register_at(
            &home,
            &file,
            None,
            &json!({"confirm": true, "profiles": ["e64"]}),
        )
        .unwrap();
        let doc = nu9ve_profiles::read(&file).unwrap().unwrap();
        let e64 = nu9ve_profiles::find(&doc, "e64").unwrap();
        assert_eq!(e64["display_name"], "Erik");
        assert_eq!(e64["aliases"], json!(["erikiado64"]));
        assert_eq!(
            e64["apps"]["nove"],
            nu9ve_profiles::find(&before, "e64").unwrap()["apps"]["nove"]
        );
        assert_eq!(e64["apps"]["nuun"]["library"], json!(home));
        assert!(nu9ve_profiles::find(&doc, "nu9ve-xyz").is_none());
        let read = overview_at(&home, Some(&file)).unwrap();
        let apps = &read["profiles"][0]["apps"];
        assert_eq!(apps[0]["app"], "nove");
        assert_eq!(apps[0]["own"], false);
        assert_eq!(apps[1]["own"], true);
    }

    #[test]
    fn an_invalid_file_is_left_as_it_is() {
        let (_dir, home, file) = setup();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"{not json").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        let read = overview_at(&home, Some(&file)).unwrap();
        assert_eq!(read["file"]["state"], "invalid");
        assert_eq!(read["file"]["problem"], "not_json");
        let err = register_at(&home, &file, None, &json!({"confirm": true})).unwrap_err();
        assert_eq!(err.code, "profiles_invalid");
        assert_eq!(std::fs::read(&file).unwrap(), b"{not json");
    }

    #[test]
    fn qa_without_the_variable_never_reaches_the_persons_file() {
        if std::env::var_os(ENV).is_none() {
            assert!(file(true).is_none());
        }
    }
}
