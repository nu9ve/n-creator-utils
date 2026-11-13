//! Clipper's public release protocol: kernel exclusion, fresh dependencies and validated ACKs.

use crate::catalog::{Catalog, WritableCatalog, operation_lock};
use crate::integrations::{CLIPPER_CAPABILITY, Integrations, invoke_clipper};
use crate::tree::Root;
use nu9ve_json::{Error, Result, hex, random_bytes, require};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) struct Guard {
    _lock: crate::catalog::OperationLock,
    token: String,
    home: PathBuf,
    integrations: Integrations,
}

impl Guard {
    pub(crate) fn acquire(state: &Path) -> Result<Self> {
        let home = std::env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                Error::new(
                    "clipper_unavailable",
                    "No se pudo determinar HOME para bloquear Clipper",
                )
            })?;
        Self::acquire_inner(state, PathBuf::from(home), None)
    }

    #[cfg(test)]
    pub(crate) fn acquire_at(state: &Path, home: PathBuf) -> Result<Self> {
        let projects = home.join("Downloads/clipper-projects");
        Self::acquire_inner(state, home, Some(projects))
    }

    fn acquire_inner(state: &Path, home: PathBuf, projects: Option<PathBuf>) -> Result<Self> {
        let directory = home.join(".cache/clipper");
        let mut guard = operation_lock(
            &directory,
            ".media-access.lock",
            "clipper_busy",
            "Clipper tiene trabajo activo; se conserva el origen",
        )?;
        let lock = guard.file();
        let token = hex(&random_bytes::<32>()?);
        lock.set_len(0)
            .and_then(|()| {
                lock.write_all(
                    json!({"pid": std::process::id(), "token": token})
                        .to_string()
                        .as_bytes(),
                )
            })
            .and_then(|()| lock.flush())
            .map_err(|error| {
                Error::new(
                    "clipper_unavailable",
                    format!("No se pudo bloquear Clipper: {error}"),
                )
            })?;
        let catalog = Catalog::open(state)?;
        let integrations = Integrations::read(&catalog)?;
        if integrations.clipper_executable().is_ok() {
            WritableCatalog::open(state)?.set("clipper-known", &json!(true))?;
        }
        // Re-read the sticky discovery bit so disappearance after this point fails closed.
        let mut integrations = Integrations::read(&catalog)?;
        if let Some(projects) = projects {
            integrations.clipper_base = projects;
        }
        let (deps, _) = integrations.clipper_dependencies(TIMEOUT, Some(&token))?;
        check_capability(&deps)?;
        Ok(Self {
            _lock: guard,
            token,
            home,
            integrations,
        })
    }

    pub(crate) fn inspect(&self, paths: &[(String, Option<String>)]) -> Result<Value> {
        let (deps, _) = self
            .integrations
            .clipper_dependencies(TIMEOUT, Some(&self.token))?;
        check_capability(&deps)?;
        crate::clipper::inspect_dependencies(&self.integrations, paths, &self.home, deps)
    }

    pub(crate) fn relink(
        &self,
        state: &Path,
        catalog: &WritableCatalog,
        request: &Value,
        operation: &str,
    ) -> Result<Value> {
        let reader = Catalog::open(state)?;
        let id = text(request, "asset_id");
        let asset = reader
            .json_rows("SELECT payload FROM assets WHERE id=?", [id])?
            .into_iter()
            .next();
        let (option, argument, destination, members) =
            if let Some(asset) = asset.filter(|asset| asset["kind"] == "delivery") {
                let (path, destination, members) =
                    delivery_handoff(state, catalog, &reader, &asset, request, operation)?;
                ("--handoff", path, destination, Some(members))
            } else {
                let destination = text(request, "to").to_owned();
                ("--to", destination.clone(), destination, None)
            };
        let command = self.integrations.clipper_executable()?;
        let ack = invoke_clipper(
            &command,
            &[
                "relink",
                text(request, "project"),
                id,
                option,
                &argument,
                "--operation-id",
                operation,
            ],
            TIMEOUT,
            Some(&self.token),
        )?;
        require(
            ack["schema"] == "clipper-relink-ack/1"
                && ack["status"] == "applied"
                && ack["operation_id"] == operation
                && ack["asset_id"] == id
                && ack["revision"] == request["revision"]
                && ack["from"] == request["from"]
                && Path::new(text(&ack, "to")) == Path::new(&destination)
                && ack["still_pinned"] == json!([]),
            "clipper_ack_invalid",
            "Clipper no confirmó un reenlace completo y libre de exportaciones",
        )?;
        if let Some(members) = members {
            require(
                ack["member_targets"] == members
                    && ack["from_members"].as_array().map(Vec::len)
                        == members.as_object().map(serde_json::Map::len),
                "clipper_ack_invalid",
                "Clipper did not ACK every delivery member",
            )?;
        }
        catalog.set(&format!("clipper-ack:{operation}"), &ack)?;
        Ok(ack)
    }
}

fn check_capability(deps: &Value) -> Result<()> {
    require(
        deps["integration"] == "never_configured"
            || deps["capabilities"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item == CLIPPER_CAPABILITY)),
        "clipper_upgrade_required",
        "Actualiza y reinicia Clipper para liberar media con exclusión coordinada",
    )
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

/// Locate the exact verified delivery and persist a stable handoff before invoking Clipper.
fn delivery_handoff(
    state: &Path,
    catalog: &WritableCatalog,
    reader: &Catalog,
    asset: &Value,
    request: &Value,
    operation: &str,
) -> Result<(String, String, Value)> {
    let mut location_id = None;
    for volume in reader.volumes()? {
        let root = Path::new(text(&volume, "root"));
        let rows = reader.json_rows(
            "SELECT payload FROM copies WHERE job_id=? AND volume_id=?",
            [text(asset, "job_id"), text(&volume, "id")],
        )?;
        if rows.iter().any(|copy| {
            asset["paths"]
                .as_array()
                .is_some_and(|paths| paths.contains(&copy["path"]))
                && root.join(text(copy, "destination")) == Path::new(text(request, "to"))
        }) {
            location_id = Some(volume["id"].clone());
        }
    }
    let volume_id = location_id.ok_or_else(|| {
        Error::new(
            "missing_receipt",
            "Delivery replacement has no recorded location",
        )
    })?;
    let key = format!("clipper-handoff:{operation}");
    let document = if let Some(intent) = catalog.get(&key)? {
        require(
            &intent["request"] == request,
            "operation_conflict",
            "Relink operation reused with another payload",
        )?;
        intent["document"].clone()
    } else {
        let document = crate::handoff::handoff(
            reader,
            &json!({
                "asset_ids": [asset["id"]], "volume_id": volume_id,
            }),
        )?;
        catalog.set(&key, &json!({"request": request, "document": document}))?;
        document
    };
    let rel = format!("handoffs/{operation}.json");
    let location = &document["assets"][0]["locations"][0];
    let volume = reader.volume(text(location, "volume_id"))?;
    let root = Path::new(text(&volume, "root"));
    let target = root.join(crate::rules::relative(text(location, "relative_path"))?);
    let mut members = serde_json::Map::new();
    for member in location["members"].as_array().into_iter().flatten() {
        members.insert(
            text(member, "original_path").to_owned(),
            json!(root.join(crate::rules::relative(text(
                member,
                "volume_relative_path"
            ))?)),
        );
    }
    let state = Root::open(state)?;
    state.write_json(&rel, &document)?;
    Ok((
        state.path.join(rel).display().to_string(),
        target.display().to_string(),
        Value::Object(members),
    ))
}
