//! Coordinate completed-session project metadata. The Stalker producer alone writes its manifest.

use crate::catalog::{Catalog, OperationLock, WritableCatalog, sql, writer_lock};
use crate::clipper_release::Guard;
use crate::integrations::{Integrations, STALKER_CAPABILITY};
use crate::volumes::{Backend, truthy};
use nu9ve_json::{Error, Result, digest, hex, require};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::os::fd::AsRawFd;
use std::path::{Component, Path};
use std::time::Duration;

pub(crate) fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

pub(crate) fn uuid(value: &Value) -> Result<String> {
    value
        .as_str()
        .and_then(crate::captures::python_uuid)
        .ok_or_else(|| Error::new("operation_failed", "badly formed hexadecimal UUID string"))
}

pub(crate) fn is_sha(value: &Value) -> bool {
    value.as_str().is_some_and(|sha| {
        sha.len() == 64
            && sha
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}

pub(crate) fn validate_project(project: &Value) -> Result<()> {
    require(
        project.as_object().is_some_and(|v| v.len() == 3)
            && project["version"].as_u64() == Some(1)
            && ["name", "country"]
                .iter()
                .all(|key| (1..=120).contains(&text(project, key).chars().count())),
        "invalid_request",
        "project must be {version:1,name,country}",
    )
}

pub(crate) fn manifest_at(path: &Path) -> Result<(Value, String)> {
    let raw = crate::tree::read_bytes(path, "manifest.json", 1024 * 1024)?.ok_or_else(|| {
        Error::new(
            "invalid_bundle",
            "Requires a completed supported Stalker session",
        )
    })?;
    let value: Value = serde_json::from_slice(&raw)
        .map_err(|error| Error::new("operation_failed", error.to_string()))?;
    require(
        value["producer"] == "stalker"
            && matches!(value["schemaVersion"].as_u64(), Some(1..=3))
            && value["status"] == "completed",
        "invalid_bundle",
        "Requires a completed supported Stalker session",
    )?;
    Ok((value, hex(&Sha256::digest(&raw))))
}

pub(crate) fn unchanged_except_project(before: &Value, after: &Value) -> bool {
    match (before.as_object(), after.as_object()) {
        (Some(before), Some(after)) => before
            .iter()
            .filter(|(key, _)| *key != "project")
            .eq(after.iter().filter(|(key, _)| *key != "project")),
        _ => false,
    }
}

pub(crate) struct Coordinator {
    pub(crate) guard: Guard,
    writer: OperationLock,
    integrations: Integrations,
}

impl Coordinator {
    pub(crate) fn acquire(home: &Path, guard: Guard) -> Result<Self> {
        let writer = writer_lock(home)?;
        let integrations = Integrations::read(&Catalog::open(home)?)?;
        Ok(Self {
            guard,
            writer,
            integrations,
        })
    }

    pub(crate) fn producer(&self, action: &str, request: &Value) -> Result<Value> {
        let (editor, token) = self.guard.inherited();
        let writer = self.writer.fd();
        self.integrations.stalker_command(
            action,
            request,
            Duration::from_secs(60),
            token,
            &[editor.as_raw_fd(), writer.as_raw_fd()],
        )
    }

    pub(crate) fn check_capability(&self) -> Result<()> {
        let result = self.producer("capabilities", &json!({}))?;
        require(
            result["schema"] == "stalker-capabilities/1"
                && result["capabilities"]
                    .as_array()
                    .is_some_and(|caps| caps.iter().any(|cap| cap == STALKER_CAPABILITY)),
            "stalker_upgrade_required",
            format!(
                "Update/restart Stalker with {STALKER_CAPABILITY} (docs/contracts/stalker-retitle-v1.md); no manifest was changed"
            ),
        )
    }
}

fn blockers(catalog: &Catalog, coordinator: &Coordinator, sid: &str, path: &str) -> Result<()> {
    let db = catalog
        .db()
        .ok_or_else(|| Error::new("catalog_unavailable", "Catalog missing"))?;
    let exists = |query: &str| {
        db.query_row(query, [sid], |row| row.get::<_, bool>(0))
            .map_err(sql)
    };
    require(
        !exists("SELECT EXISTS(SELECT 1 FROM sessions WHERE lower(session_id)=?)")?,
        "retitle_archived",
        "A verified archived revision exists; keep the manifest immutable and use catalog tags",
    )?;
    require(
        !exists(
            "SELECT EXISTS(SELECT 1 FROM jobs, json_each(jobs.plan,'$.units') AS unit WHERE lower(json_extract(unit.value,'$.session_id'))=?)",
        )?,
        "retitle_plan_active",
        "An ingest job references the session; resolve it before retitling",
    )?;
    for plan in catalog.json_rows(
        "SELECT value FROM metadata WHERE key LIKE 'reviewed-plan:%'",
        [],
    )? {
        if !truthy(&plan["cancelled"])
            && plan["sessions"]
                .as_array()
                .is_some_and(|sessions| sessions.iter().any(|session| session == sid))
        {
            return Err(Error::new(
                "retitle_plan_active",
                format!(
                    "Cancel the reviewed plan before retitling: {}",
                    text(&plan, "id")
                ),
            ));
        }
    }
    require(
        !catalog
            .json_rows("SELECT value FROM metadata WHERE key LIKE 'release:%'", [])?
            .iter()
            .any(|plan| plan["state"] != "completed"),
        "retitle_plan_active",
        "Resolve all active release plans before retitling",
    )?;
    let inspection = coordinator.guard.inspect(&[(path.to_owned(), None)])?;
    let blockers: Vec<_> = inspection["blockers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    require(
        blockers.is_empty(),
        "retitle_dependencies",
        blockers.join("; "),
    )?;
    for project in inspection["dependencies"]["projects"]
        .as_array()
        .into_iter()
        .flatten()
    {
        for asset in project["assets"].as_array().into_iter().flatten() {
            let paths = std::iter::once(&asset["path"])
                .chain(asset["items"].as_array().into_iter().flatten());
            require(
                !paths
                    .filter_map(Value::as_str)
                    .any(|value| crate::clipper::overlaps(path, value)),
                "retitle_dependencies",
                "Clipper references this session; a new revision requires explicit consumer adoption",
            )?;
        }
    }
    Ok(())
}

pub(crate) fn matching_result(result: &Value, request: &Value) -> bool {
    result["schema"] == "stalker-retitle-result/1"
        && result["operation_id"] == request["operation_id"]
        && result["session_id"] == request["session_id"]
}

pub(crate) fn safe_target(target: &str, source: &Path) -> bool {
    let path = Path::new(target);
    !target.contains('\0')
        && path.is_absolute()
        && !path.components().any(|c| c == Component::ParentDir)
        && path.parent() == source.parent()
        && path.extension().is_some_and(|ext| ext == "stalker")
        && !path.ancestors().any(Path::is_symlink)
}

pub fn retitle(home: &Path, backend: &dyn Backend, params: &Value) -> Result<Value> {
    retitle_with(home, backend, params, || Guard::acquire(home))
}

fn retitle_with(
    home: &Path,
    backend: &dyn Backend,
    params: &Value,
    acquire: impl FnOnce() -> Result<Guard>,
) -> Result<Value> {
    let operation = uuid(&params["operation_id"])?;
    let sid = uuid(&params["session_id"])?;
    let project = &params["project"];
    validate_project(project)?;
    let path = text(params, "path");
    let source = Path::new(path);
    require(
        source.is_absolute()
            && !source.ancestors().any(Path::is_symlink)
            && !path.contains('\0')
            && !source.components().any(|c| c == Component::ParentDir),
        "unsafe_path",
        "Absolute session path required",
    )?;
    let expected = &params["expected_manifest_sha256"];
    require(
        is_sha(expected),
        "invalid_request",
        "Expected raw manifest SHA-256 required",
    )?;
    let home =
        std::path::absolute(home).map_err(|e| Error::new("operation_failed", e.to_string()))?;
    let request = json!({"schema": STALKER_CAPABILITY, "operation_id": operation, "path": path,
        "session_id": sid, "expected_manifest_sha256": expected, "project": project, "catalog_id": home});
    // Validate the parent so a renamed session can recover and the sandbox cannot access unrelated sessions.
    let parent = source
        .parent()
        .ok_or_else(|| Error::new("unsafe_path", "Absolute session path required"))?;
    backend.describe(parent, false)?;
    let coordinator = Coordinator::acquire(&home, acquire()?)?;
    let writer = WritableCatalog::open(&home)?;
    let catalog = Catalog::open(&home)?;
    let key = format!("retitle:{operation}");
    let mut prior = writer.get(&key)?;
    if let Some(prior) = &prior {
        require(
            prior["request"] == request,
            "operation_conflict",
            "operation_id reused with different payload",
        )?;
        if prior["state"] == "applied" {
            return Ok(prior.clone());
        }
    }
    require(
        !catalog
            .json_rows("SELECT value FROM metadata WHERE key LIKE 'retitle:%'", [])?
            .iter()
            .any(|op| {
                op["request"]["session_id"] == sid
                    && op["request"]["operation_id"] != operation
                    && op["state"] != "applied"
            }),
        "retitle_pending",
        "Recover the existing operation first",
    )?;
    coordinator.check_capability()?;
    let check = |path| blockers(&catalog, &coordinator, &sid, path);
    if prior.is_none() {
        check(path)?;
        let (manifest, sha) = manifest_at(source)?;
        require(
            uuid(&manifest["sessionID"])? == sid && *expected == sha,
            "revision_mismatch",
            "Stalker session changed; review metadata again",
        )?;
        let pending = json!({"schema": "nuun-retitle-operation/1", "request": request, "previous_manifest": manifest,
            "state": "pending", "updated_at": nu9ve_time::now_utc()});
        writer.set(&key, &pending)?;
        prior = Some(pending);
    }
    let mut prior = prior.unwrap();
    let mut result = coordinator.producer("status", &request)?;
    require(
        matching_result(&result, &request),
        "stalker_invalid",
        "Invalid producer status; operation remains pending",
    )?;
    match text(&result, "status") {
        "pending" => {
            let target = text(&result, "path");
            require(
                safe_target(target, source)
                    && result["project"] == *project
                    && result["previous_manifest_sha256"] == *expected
                    && is_sha(&result["manifest_sha256"]),
                "stalker_invalid",
                "Invalid pending journal result; operation remains pending",
            )?;
            check(path)?;
            if target != path {
                check(target)?;
            }
            let recovered = coordinator.producer("apply", &request)?;
            require(
                recovered["path"] == target
                    && recovered["manifest_sha256"] == result["manifest_sha256"],
                "stalker_invalid",
                "Recovery result differs from the pending journal; operation remains pending",
            )?;
            result = recovered;
        }
        "not_found" => {
            check(path)?;
            let (manifest, sha) = manifest_at(source)?;
            require(
                *expected == sha && uuid(&manifest["sessionID"])? == sid,
                "revision_mismatch",
                "Manifest changed while recovering retitle",
            )?;
            result = coordinator.producer("apply", &request)?;
        }
        "applied" => {}
        _ => {
            return Err(Error::new(
                "retitle_recovery_required",
                "Producer operation is unresolved; retain session and recover in Stalker",
            ));
        }
    }
    let target = text(&result, "path");
    require(
        matching_result(&result, &request)
            && result["status"] == "applied"
            && result["previous_manifest_sha256"] == *expected
            && result["project"] == *project
            && safe_target(target, source),
        "stalker_invalid",
        "Stalker did not publish a matching result; operation remains pending",
    )?;
    let (manifest, sha) = manifest_at(Path::new(target))?;
    require(
        result["manifest_sha256"] == sha
            && uuid(&manifest["sessionID"])? == sid
            && manifest["project"] == *project
            && unchanged_except_project(&prior["previous_manifest"], &manifest),
        "revision_mismatch",
        "Published manifest does not match the producer result",
    )?;
    let mut invalidation = result.clone();
    invalidation["manifest_digest"] = json!(digest(&manifest));
    prior["state"] = json!("applied");
    prior["result"] = result;
    prior["updated_at"] = json!(nu9ve_time::now_utc());
    let tx = writer.db().unchecked_transaction().map_err(sql)?;
    writer.set(&format!("retitle-result:{sid}"), &invalidation)?;
    writer.set(&key, &prior)?;
    tx.commit().map_err(sql)?;
    Ok(prior)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::volumes::test_backend::{Setup, setup};
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    pub(crate) fn producer_fixture(s: &Setup) {
        let root = s.archive.parent().unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/contracts/stalker-producer.py")
            .canonicalize()
            .unwrap();
        let producer = s.home.join("stalker");
        std::fs::write(
            &producer,
            format!(
                r#"#!/bin/sh
export STALKER_RETITLE_FIXTURE_ROOT='{}'
exec python3 '{}' "$@"
"#,
                root.display(),
                fixture.display()
            ),
        )
        .unwrap();
        let clipper = s.home.join("clipper");
        std::fs::write(&clipper, r#"#!/bin/sh
echo '{"schema":"clipper-library-deps/1","capabilities":["media-release-lock/1"],"offline_projects":[],"projects":[]}'
"#).unwrap();
        let writer = WritableCatalog::open(&s.home).unwrap();
        for (key, path) in [
            ("stalker-executable", producer),
            ("clipper-executable", clipper),
        ] {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            writer.set(key, &json!(path)).unwrap();
        }
    }

    pub(crate) fn capture(s: &Setup) -> (PathBuf, Value, String) {
        let root = s.card.join("fixture.stalker");
        std::fs::create_dir(&root).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/contracts/stalker-v3.json");
        std::fs::copy(fixture, root.join("manifest.json")).unwrap();
        std::fs::write(root.join("screen.mov"), b"fixture video").unwrap();
        let (manifest, sha) = manifest_at(&root).unwrap();
        (root, manifest, sha)
    }

    fn run(s: &Setup, params: &Value) -> Result<Value> {
        retitle_with(&s.home, &s.backend, params, || {
            Guard::acquire_at(&s.home, s.home.join("clipper-home"))
        })
    }

    #[test]
    fn producer_keeps_both_locks_after_the_coordinator_is_killed() {
        use std::process::{Command, Stdio};
        use std::time::Instant;
        const CHILD: &str = "NUUN_RETITLE_TEST_STATE";
        if let Some(home) = std::env::var_os(CHILD) {
            let home = PathBuf::from(home);
            let coordinator = Coordinator::acquire(
                &home,
                Guard::acquire_at(&home, home.join("clipper-home")).unwrap(),
            )
            .unwrap();
            let request: Value =
                serde_json::from_slice(&std::fs::read(home.join("request.json")).unwrap()).unwrap();
            coordinator.producer("apply", &request).unwrap();
            return;
        }
        let s = setup();
        producer_fixture(&s);
        let (path, manifest, sha) = capture(&s);
        let id = "9e20748f-b1da-419f-9a54-b46f506eb59d";
        let request = json!({"schema":STALKER_CAPABILITY,"operation_id":id,"path":path,
            "session_id":uuid(&manifest["sessionID"]).unwrap(),"expected_manifest_sha256":sha,
            "catalog_id":s.home,"project":{"version":1,"name":"nuevo","country":"mx"}});
        std::fs::write(s.home.join("request.json"), nu9ve_json::canonical(&request)).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "retitle::tests::producer_keeps_both_locks_after_the_coordinator_is_killed",
            ])
            .env(CHILD, &s.home)
            .env("STALKER_RETITLE_FIXTURE_PAUSE", "1")
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let root = s.archive.parent().unwrap();
        let wait_for = |path: &Path| {
            let start = Instant::now();
            while !path.exists() && start.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(10));
            }
            path.exists()
        };
        let started = wait_for(&root.join("producer-entered"));
        let _ = child.kill();
        child.wait().unwrap();
        let writer_busy = writer_lock(&s.home).err().map(|error| error.code);
        let editor_busy = Guard::acquire_at(&s.home, s.home.join("clipper-home"))
            .err()
            .map(|error| error.code);
        std::fs::write(root.join("producer-resume"), b"").unwrap();
        assert!(started, "Stalker did not reach the fixture checkpoint");
        assert_eq!(writer_busy.as_deref(), Some("job_busy"));
        assert_eq!(editor_busy.as_deref(), Some("clipper_busy"));
        assert!(wait_for(&root.join(format!("{id}.result.json"))));
        let start = Instant::now();
        loop {
            if let Ok(_writer) = writer_lock(&s.home) {
                let _editor = Guard::acquire_at(&s.home, s.home.join("clipper-home")).unwrap();
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn retitle_checks_plans_and_invalidates_old_ingest_metadata_after_producer_ack() {
        let s = setup();
        producer_fixture(&s);
        let (root, manifest, sha) = capture(&s);
        let sid = uuid(&manifest["sessionID"]).unwrap();
        let request = json!({"operation_id":"f1cb402a-5855-4f29-83ba-937a3d9ec088",
            "path":root,"session_id":sid,"expected_manifest_sha256":sha,
            "project":{"version":1,"name":"nuevo","country":"mx"}});
        let writer = WritableCatalog::open(&s.home).unwrap();
        writer
            .set(
                "reviewed-plan:fixture",
                &json!({"id":"fixture","sessions":[sid],"cancelled":false}),
            )
            .unwrap();
        assert_eq!(run(&s, &request).unwrap_err().code, "retitle_plan_active");
        assert_eq!(manifest_at(&root).unwrap().1, sha);
        crate::settings::cancel_plan(&s.home, &json!({"plan_id":"fixture"})).unwrap();

        let result = run(&s, &request).unwrap();
        assert_eq!(result["state"], "applied");
        assert_eq!(result, run(&s, &request).unwrap());
        assert_eq!(
            std::fs::read(root.join("screen.mov")).unwrap(),
            b"fixture video"
        );
        let (after, _) = manifest_at(&root).unwrap();
        assert!(unchanged_except_project(&manifest, &after));
        assert_eq!(after["project"], request["project"]);
        let catalog = Catalog::open(&s.home).unwrap();
        assert_eq!(
            catalog
                .get(&format!("retitle-result:{sid}"))
                .unwrap()
                .unwrap()["manifest_digest"],
            digest(&after)
        );
        let old = json!({"units":[{"session_id":sid,"capture_manifest_digest":digest(&manifest)}]});
        assert_eq!(
            crate::planner::check_retitle(&catalog, &old)
                .unwrap_err()
                .code,
            "plan_changed"
        );

        let mut different = request.clone();
        different["project"]["name"] = json!("otro");
        assert_eq!(run(&s, &different).unwrap_err().code, "operation_conflict");
        // Simulate losing the coordinator after Stalker persisted its ACK.
        let mut pending = result.clone();
        pending["state"] = json!("pending");
        pending.as_object_mut().unwrap().remove("result");
        writer
            .set(
                &format!("retitle:{}", request["operation_id"].as_str().unwrap()),
                &pending,
            )
            .unwrap();
        let mut concurrent = request.clone();
        concurrent["operation_id"] = json!("cb2124ca-a839-4c55-b58c-d6600d81fa60");
        assert_eq!(run(&s, &concurrent).unwrap_err().code, "retitle_pending");
        assert_eq!(run(&s, &request).unwrap()["result"], result["result"]);
    }
}
