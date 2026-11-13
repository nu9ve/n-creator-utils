//! Publish a Stalker project revision from an isolated, verified archive copy.

use crate::catalog::{Catalog, WritableCatalog, sql};
use crate::clipper_release::Guard;
use crate::release::checked_hash;
use crate::retitle::{
    Coordinator, is_sha, manifest_at, matching_result, safe_target, text, unchanged_except_project,
    uuid, validate_project,
};
use crate::rules::relative;
use crate::transfer::{Hook, Target, Transfer};
use crate::tree::Root;
use crate::volumes::{self, Backend};
use nu9ve_json::{Error, NAMESPACE_URL, Result, canonical, digest, require, uuid5};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn paths(asset: &Value) -> Result<Vec<&str>> {
    asset["paths"]
        .as_array()
        .ok_or_else(|| Error::new("missing_receipt", "Complete revision receipts required"))?
        .iter()
        .map(|p| relative(p.as_str().unwrap_or_default()))
        .collect()
}

fn bundle_prefix(asset: &Value) -> Result<String> {
    let bundle = text(asset, "bundle");
    if bundle == "." {
        Ok(String::new())
    } else {
        Ok(format!("{}/", relative(bundle)?))
    }
}

fn inner<'a>(path: &'a str, prefix: &str) -> Result<&'a str> {
    path.strip_prefix(prefix)
        .ok_or_else(|| Error::new("revision_mismatch", "Member outside revision bundle"))
}

fn inventory(catalog: &Catalog, asset: &Value, volume: &Value) -> Result<BTreeMap<String, Value>> {
    let paths = paths(asset)?;
    let rows: BTreeMap<_, _> = catalog
        .json_rows(
            "SELECT payload FROM copies WHERE job_id=? AND volume_id=?",
            [text(asset, "job_id"), text(volume, "id")],
        )?
        .into_iter()
        .filter(|copy| paths.contains(&text(copy, "path")))
        .map(|copy| (text(&copy, "path").to_owned(), copy))
        .collect();
    require(
        rows.len() == paths.len(),
        "missing_receipt",
        "Complete revision receipts required",
    )?;
    let hashes: Vec<_> = paths
        .iter()
        .map(|path| json!({"path": path, "sha256": rows[*path]["sha256"]}))
        .collect();
    require(
        asset["revision"] == digest(&json!(hashes)),
        "revision_mismatch",
        "Receipt inventory differs from the asset revision",
    )?;
    Ok(rows)
}

fn verify_membership(root: &Path, rows: &BTreeMap<String, Value>, prefix: &str) -> Result<()> {
    let expected = rows
        .keys()
        .map(|p| inner(p, prefix))
        .collect::<Result<BTreeSet<_>>>()?;
    let scan = crate::scan::scan(root, false, false, false)?;
    require(
        scan.files
            .iter()
            .map(|file| text(file, "path"))
            .collect::<BTreeSet<_>>()
            == expected,
        "revision_mismatch",
        "Revision membership changed",
    )
}

fn verify(
    backend: &dyn Backend,
    asset: &Value,
    volume: &Value,
    tree: &Root,
    rows: &BTreeMap<String, Value>,
) -> Result<String> {
    volumes::check(backend, volume)?;
    let prefix = bundle_prefix(asset)?;
    let manifest = rows
        .get(&format!("{prefix}manifest.json"))
        .ok_or_else(|| Error::new("missing_receipt", "Complete revision receipts required"))?;
    let root = text(manifest, "destination")
        .strip_suffix("/manifest.json")
        .ok_or_else(|| Error::new("revision_mismatch", "Member outside revision bundle"))?;
    relative(root)?;
    for (path, row) in rows {
        let destination = text(row, "destination");
        require(
            destination == format!("{root}/{}", inner(path, &prefix)?),
            "revision_mismatch",
            "Member outside revision bundle",
        )?;
        require(
            checked_hash(tree, destination)?
                .is_some_and(|(sha, stamp)| row["sha256"] == sha && row["bytes"] == stamp["size"]),
            "revision_mismatch",
            "Revision bytes changed",
        )?;
    }
    verify_membership(&tree.path.join(root), rows, &prefix)?;
    Ok(root.to_owned())
}

pub fn publish(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    publish_with(home, backend, params, || Guard::acquire(home), &|_, _| {
        Ok(())
    })
}

fn publish_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    acquire: impl FnOnce() -> Result<Guard>,
    hook: Hook<'_>,
) -> Result<Value> {
    let operation = uuid(&params["operation_id"])?;
    let project = &params["project"];
    validate_project(project)?;
    let asset_id = text(params, "asset_id");
    let expected = &params["expected_revision"];
    let home = std::path::absolute(home)
        .map_err(|error| Error::new("operation_failed", error.to_string()))?;
    let catalog = Catalog::open(&home)?;
    let archive = catalog.volume(text(params, "archive"))?;
    require(
        archive["role"] == "archive",
        "invalid_role",
        "Publish to a registered archive",
    )?;
    let request = json!({"operation_id": operation, "asset_id": asset_id, "expected_revision": expected, "project": project, "archive": archive["id"]});
    let key = format!("revision-publication:{operation}");
    let coordinator = Coordinator::acquire(&home, acquire()?)?;
    let writer = WritableCatalog::open(&home)?;
    let prior = writer.get(&key)?;
    if let Some(prior) = &prior {
        require(
            prior["request"] == request,
            "operation_conflict",
            "operation_id reused with another payload",
        )?;
        if prior["state"] == "published" {
            return Ok(prior.clone());
        }
    }
    let current = catalog
        .json_rows("SELECT payload FROM assets WHERE id=?", [asset_id])?
        .into_iter()
        .next()
        .ok_or_else(|| Error::new("unknown_asset", "Asset not found"))?;
    require(
        current["kind"] == "stalker_session" && current["revision"] == *expected,
        "revision_mismatch",
        "Review the current Stalker asset revision",
    )?;
    require(
        !catalog
            .json_rows(
                "SELECT value FROM metadata WHERE key LIKE 'revision-publication:%'",
                [],
            )?
            .iter()
            .any(|value| {
                value["state"] != "published"
                    && value["request"]["asset_id"] == asset_id
                    && value["request"]["operation_id"] != operation
            }),
        "revision_pending",
        "Recover the existing publication with its operation ID",
    )?;
    coordinator.check_capability()?;
    volumes::check(backend, &archive)?;
    let tree = Root::open(Path::new(text(&archive, "root")))?;
    let parent = format!(
        "{}/revisions/{operation}",
        relative(text(&archive, "prefix"))?
    );
    let staging = format!("{parent}/revision.stalker");
    let (mut op, rows) = if let Some(prior) = prior {
        require(
            prior["previous_asset"] == current
                && prior["staging"] == staging
                && prior["parent"] == parent,
            "revision_mismatch",
            "Publication inventory differs from its journal",
        )?;
        let rows = serde_json::from_value(prior["previous_copies"].clone())
            .map_err(|error| Error::new("operation_failed", error.to_string()))?;
        (prior, rows)
    } else {
        let rows = inventory(&catalog, &current, &archive)?;
        let source = verify(backend, &current, &archive, &tree, &rows)?;
        let (manifest, sha) = manifest_at(&tree.path.join(source))?;
        require(
            uuid(&manifest["sessionID"])? == uuid(&current["session_id"])?,
            "revision_mismatch",
            "Archived session identity differs",
        )?;
        require(
            manifest["project"] != *project,
            "revision_mismatch",
            "Project has not changed",
        )?;
        let op = json!({"schema":"nuun-revision-publication/1", "request":request, "state":"pending",
            "previous_asset":current,"previous_copies":rows,"previous_manifest":manifest,"previous_manifest_sha256":sha,
            "staging":staging,"parent":parent,"updated_at":nu9ve_time::now_utc()});
        writer.set(&key, &op)?;
        (op, rows)
    };
    let prefix = bundle_prefix(&current)?;
    verify(backend, &current, &archive, &tree, &rows)?;
    let stage_path = tree.path.join(&staging);
    let producer_request = json!({"schema":crate::integrations::STALKER_CAPABILITY,
        "operation_id":operation,"path":stage_path,"session_id":uuid(&current["session_id"])?,
        "expected_manifest_sha256":op["previous_manifest_sha256"],"project":project,"catalog_id":home});
    volumes::check(backend, &archive)?;
    tree.parent(&staging, true)?;
    let mut status = coordinator.producer("status", &producer_request)?;
    require(
        matching_result(&status, &producer_request)
            && matches!(
                status["status"].as_str(),
                Some("not_found" | "pending" | "applied")
            ),
        "stalker_invalid",
        "Unresolved producer status; retain both revisions",
    )?;
    let valid_ack = |result: &Value| {
        safe_target(text(result, "path"), &stage_path)
            && result["previous_manifest_sha256"] == producer_request["expected_manifest_sha256"]
            && result["project"] == *project
            && is_sha(&result["manifest_sha256"])
    };
    if status["status"] == "pending" {
        require(
            valid_ack(&status),
            "stalker_invalid",
            "Pending producer journal differs from the publication",
        )?;
    }
    if status["status"] == "not_found" {
        let mut transfer = Transfer::new(&home, &writer, backend, &|_, _| Ok(()));
        for (path, row) in &rows {
            let rel = format!("{staging}/{}", inner(path, &prefix)?);
            transfer.copy_many(
                &tree,
                text(row, "destination"),
                &[Target {
                    root: &tree,
                    rel,
                    record: &archive,
                }],
                Some(text(row, "sha256")),
                None,
            )?;
        }
        hook("copy_verified", &op)?;
    }
    let pending = (status["status"] == "pending").then(|| status.clone());
    if status["status"] != "applied" {
        status = coordinator.producer("apply", &producer_request)?;
    }
    require(
        matching_result(&status, &producer_request)
            && status["status"] == "applied"
            && valid_ack(&status)
            && pending.is_none_or(|prior| {
                prior["path"] == status["path"]
                    && prior["manifest_sha256"] == status["manifest_sha256"]
            }),
        "stalker_invalid",
        "Producer did not ACK the isolated copy",
    )?;
    hook("producer_applied", &status)?;
    let target = Path::new(text(&status, "path"));
    let (manifest, sha) = manifest_at(target)?;
    require(
        status["manifest_sha256"] == sha
            && manifest["project"] == *project
            && unchanged_except_project(&op["previous_manifest"], &manifest),
        "revision_mismatch",
        "Only manifest.project may change",
    )?;
    verify_membership(target, &rows, &prefix)?;
    let receipt_id = uuid5(
        &NAMESPACE_URL,
        &format!("{operation}:{}", text(&archive, "id")),
    );
    let target_rel = target
        .strip_prefix(&tree.path)
        .map_err(|_| Error::new("unsafe_path", "Revision outside archive"))?
        .to_str()
        .ok_or_else(|| Error::new("unsafe_path", "Invalid revision path"))?;
    let mut copies = Vec::new();
    let mut hashes = Vec::new();
    let mut layout = Map::new();
    let mut files = Vec::new();
    for path in paths(&current)? {
        let inner = inner(path, &prefix)?;
        let dest = format!("{target_rel}/{inner}");
        let (new_sha, stamp) = checked_hash(&tree, &dest)?
            .ok_or_else(|| Error::new("revision_mismatch", "Media/events must remain identical"))?;
        let old = &rows[path];
        require(
            if inner == "manifest.json" {
                new_sha == sha
            } else {
                old["sha256"] == new_sha && old["bytes"] == stamp["size"]
            },
            "revision_mismatch",
            "Media/events must remain identical",
        )?;
        copies.push(json!({"job_id":operation,"volume_id":archive["id"],"path":path,"destination":dest,"sha256":new_sha,
            "bytes":stamp["size"],"stamp":stamp,"receipt_id":receipt_id,"verified_at":nu9ve_time::now_utc()}));
        hashes.push(json!({"path":path,"sha256":new_sha}));
        files.push(json!({"path":path,"unit_id":asset_id,"stamp":stamp,"bundle":current["bundle"],"evidence":[]}));
        let archive_rel = dest
            .strip_prefix(text(&archive, "prefix"))
            .and_then(|p| p.strip_prefix('/'))
            .ok_or_else(|| Error::new("invalid_plan", "Revision outside archive prefix"))?;
        let name = target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        layout.insert(path.into(), json!({"archive":archive_rel,
            "work":format!("{}_raw/{}/revisions/{operation}/{name}/{inner}", text(&current,"profile"),text(&current,"project"))}));
    }
    verify(backend, &current, &archive, &tree, &rows)?;
    let mut new = current.clone();
    new["job_id"] = json!(operation);
    new["revision"] = json!(digest(&json!(hashes)));
    new["capture_manifest_digest"] = json!(digest(&manifest));
    new["source"] = json!(target);
    let unit = Value::Object(
        new.as_object()
            .unwrap()
            .iter()
            .filter(|(key, _)| {
                !["job_id", "revision", "source", "project_id"].contains(&key.as_str())
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    );
    let job = crate::reads::job(&catalog, &json!({"job_id":current["job_id"]}))?;
    let mut plan = job["plan"].clone();
    let old_expected = plan["expected_work_id"].clone();
    plan.as_object_mut().unwrap().remove("digest");
    let changes = json!({"id":operation,"source":target,"cleanup":false,"units":[unit],
        "archive_id":archive["id"],"work_id":null,"archive_only_confirmed":true,"work_delivery_status":"pending",
        "expected_work_id":old_expected,"files":files,"layout":layout});
    plan.as_object_mut()
        .unwrap()
        .extend(changes.as_object().unwrap().clone());
    plan["digest"] = json!(digest(&plan));
    crate::planner::validate_plan(&plan)?;
    let mut receipt_unit = unit;
    receipt_unit["project_id"] = current["project_id"].clone();
    let receipt = json!({"schema":"nu9ve-transfer-receipt/1","receipt_id":receipt_id,"job_id":operation,"plan_digest":plan["digest"],
        "volume_id":archive["id"],"verified_at":nu9ve_time::now_utc(),"files":copies,"units":[receipt_unit],"plan":plan,
        "country":plan["country"],"city":plan["city"],"work_delivery_status":"pending",
        "revision_publication":{"previous_asset":current,"asset":new,"operation_id":operation}});
    volumes::check(backend, &archive)?;
    tree.write_json(&format!(".ncreator/receipts/{operation}.json"), &receipt)?;
    hook("receipt_durable", &receipt)?;

    op["state"] = json!("published");
    op["asset"] = new.clone();
    op["result"] = status;
    op["updated_at"] = json!(nu9ve_time::now_utc());
    let payload =
        |value: &Value| String::from_utf8(canonical(value)).expect("canonical JSON is UTF-8");
    let tx = writer.db().unchecked_transaction().map_err(sql)?;
    for asset in [&current, &new] {
        tx.execute(
            "INSERT OR IGNORE INTO asset_revisions VALUES(?,?,?)",
            (asset_id, text(asset, "revision"), payload(asset)),
        )
        .map_err(sql)?;
    }
    let now = nu9ve_time::now_utc();
    tx.execute(
        "INSERT INTO jobs VALUES(?,?,?,'recovered',NULL,?,?)",
        (
            &operation,
            text(&plan, "digest"),
            payload(&plan),
            &now,
            &now,
        ),
    )
    .map_err(sql)?;
    for copy in &copies {
        tx.execute(
            "INSERT INTO copies VALUES(?,?,?,?,?)",
            (
                &operation,
                text(&archive, "id"),
                text(copy, "path"),
                text(copy, "sha256"),
                payload(copy),
            ),
        )
        .map_err(sql)?;
        tx.execute(
            "INSERT INTO members(job_id,relative_path,sha256) VALUES(?,?,?)",
            (&operation, text(copy, "path"), text(copy, "sha256")),
        )
        .map_err(sql)?;
    }
    tx.execute("INSERT INTO asset_jobs VALUES(?,?)", (asset_id, &operation))
        .map_err(sql)?;
    tx.execute(
        "INSERT OR IGNORE INTO sessions VALUES(?,?,?)",
        (
            text(&new, "session_id"),
            text(&new, "capture_manifest_digest"),
            asset_id,
        ),
    )
    .map_err(sql)?;
    tx.execute(
        "UPDATE assets SET job_id=?,payload=? WHERE id=?",
        (&operation, payload(&new), asset_id),
    )
    .map_err(sql)?;
    tx.execute(
        "UPDATE work SET state='pending',target=NULL,updated_at=? WHERE asset_id=?",
        (&now, asset_id),
    )
    .map_err(sql)?;
    writer.set(&key, &op)?;
    tx.commit().map_err(sql)?;
    hook("published", &op)?;
    Ok(op)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::retitle::tests::{capture, producer_fixture};
    use crate::volumes::test_backend::{Setup, setup};

    pub(crate) fn fixture() -> (Setup, Value, Value) {
        let s = setup();
        producer_fixture(&s);
        WritableCatalog::open(&s.home)
            .unwrap()
            .set("retain-stalker-source", &json!(true))
            .unwrap();
        let (bundle, mut manifest, _) = capture(&s);
        manifest.as_object_mut().unwrap().remove("captureTimeZone");
        std::fs::write(bundle.join("manifest.json"), canonical(&manifest)).unwrap();
        let ingest = crate::planner::build_plan(&s.home, &s.backend, &json!({
            "source": s.card,"source_kind":"stalker","archive":"MEDIA","work":"ESSD","project":"intro",
            "cleanup":false,
        })).unwrap();
        crate::transfer::apply(
            &s.home,
            &s.backend,
            &ingest,
            text(&ingest, "digest"),
            &|_, _| Ok(()),
        )
        .unwrap();
        let catalog = Catalog::open(&s.home).unwrap();
        let asset = catalog
            .json_rows("SELECT payload FROM assets", [])
            .unwrap()
            .remove(0);
        let params = json!({"asset_id":asset["id"],"expected_revision":asset["revision"],
            "operation_id":"d2570f57-55e8-41b5-b1f3-f8e6777412b1","archive":"MEDIA",
            "project":{"version":1,"name":"revision nueva","country":"mx"}});
        (s, asset, params)
    }

    fn run(s: &Setup, params: &Value, hook: Hook<'_>) -> Result<Value> {
        publish_with(
            &s.home,
            &s.backend,
            params,
            || Guard::acquire_at(&s.home, s.home.join("clipper-home")),
            hook,
        )
    }

    #[test]
    fn publication_reconciles_after_the_receipt_and_preserves_the_previous_revision() {
        let (s, old, params) = fixture();
        let original_receipt = s
            .archive
            .join(format!(".ncreator/receipts/{}.json", text(&old, "job_id")));
        let original = std::fs::read(&original_receipt).unwrap();
        let operation = text(&params, "operation_id");
        let error = run(&s, &params, &|stage, _| {
            require(
                stage != "receipt_durable",
                "fixture_interrupted",
                "lost coordinator",
            )
        })
        .unwrap_err();
        assert_eq!(error.code, "fixture_interrupted");
        let catalog = Catalog::open(&s.home).unwrap();
        let pending = catalog
            .get(&format!("revision-publication:{operation}"))
            .unwrap()
            .unwrap();
        assert_eq!(pending["state"], "pending");
        assert_eq!(
            catalog.json_rows("SELECT payload FROM assets", []).unwrap(),
            std::slice::from_ref(&old)
        );
        let mut alternate = params.clone();
        alternate["operation_id"] = json!("81c7821f-724d-4817-948e-ce9d8e34b502");
        assert_eq!(
            run(&s, &alternate, &|_, _| Ok(())).unwrap_err().code,
            "revision_pending"
        );
        let result = run(&s, &params, &|_, _| Ok(())).unwrap();
        assert_eq!(result["state"], "published");
        assert_eq!(result, run(&s, &params, &|_, _| Ok(())).unwrap());
        assert_eq!(std::fs::read(original_receipt).unwrap(), original);
        assert_ne!(result["asset"]["revision"], old["revision"]);
        let fresh = Path::new(text(&result["result"], "path"));
        assert_eq!(
            std::fs::read(fresh.join("screen.mov")).unwrap(),
            b"fixture video"
        );
        assert_eq!(manifest_at(fresh).unwrap().0["project"], params["project"]);
        let history =
            crate::reads::asset_revisions(&catalog, &json!({"asset_id":old["id"]})).unwrap();
        assert_eq!(history.as_array().unwrap().len(), 2);

        let rebuilt = s.home.with_file_name("rebuilt");
        let catalog_db = crate::catalog::fixture::create(&rebuilt);
        let registered = catalog.volume("MEDIA").unwrap();
        crate::catalog::fixture::register(
            &catalog_db,
            "MEDIA",
            "archive",
            &s.archive,
            &registered["identity"],
            "001_raw",
        );
        crate::recovery::recover(&rebuilt, &s.backend, &json!({"volume":"MEDIA"})).unwrap();
        let rebuilt = Catalog::open(&rebuilt).unwrap();
        assert_eq!(
            rebuilt.json_rows("SELECT payload FROM assets", []).unwrap()[0]["revision"],
            result["asset"]["revision"]
        );
    }
}
