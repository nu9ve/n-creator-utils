# Release gates

Current verdict: **development implementation, not release ready**. Evidence is in [STATUS](STATUS.md). This checklist defines what closes that gap; it does not claim the gates passed.

Último cierre local para aceptación: [2026-10-26](releases/2026-10-26-local.md).

## Candidate requirements

| Gate | Required evidence |
| --- | --- |
| Final source snapshot | Full current Python suite, native tests and focused failure/replay tests pass; known failures documented and resolved. |
| Packaging | Fresh wheel includes `media`, excludes retired Clipper, works outside checkout. Align package/app/backend versions and record source revision. |
| Protocol | Current exported handoff validates shape and semantics; integer timestamps survive native roundtrip; consumers explicitly report adopted capabilities. |
| Data-loss boundary | Review required destinations, hashes, receipts, identity, same-device detection, symlinks, membership changes, cleanup interruption and concurrent access assumptions. No testing of destructive paths on unique personal media. |
| Camera and filesystem | Small real Sony/H6/DJI trials, external mount/reconnect and ExFAT/HFS+ behavior. Sony action-required is honest but not a completed automated-camera-cleanup release gate. |
| Catalog durability | Restore/recovery, stable human decisions, project aliases, secondary locations and recovered work ownership survive documented failure/migration scenarios. |
| Native acceptance | Idle CPU/RSS/IO/wakeups, active copy/capture contention, cancel/quit/update, event loss/wake/remount, Accessibility and first-run flows. |
| Distribution | Runtime and licenses bundled; native libraries signed correctly; Developer ID, notarization/stapling and Gatekeeper clean-user acceptance. No implicit Homebrew/system-Python/checkout dependency. |

The builder [script](../scripts/build_media_app.py) creates an ad-hoc development app or takes a supplied relocatable runtime and signing identity. It does not download/provision that runtime, notarize the app, implement an updater or certify camera behavior. Its output correctly reports `release_ready: false`.

## Release record

Create a short dated record only for a real candidate: source revision, package/app versions, runtime/license source, build command, hashes/signing results, test commands/results, physical devices/firmware/filesystems, remaining limitations and recovery instructions. Keep credentials and personal paths out of the public record.

A distributable artifact must be rebuilt after final source changes. The session's development bundles in `dist/` are snapshots; a live external Python pointer can make a smoke test non-reproducible. Do not treat those bundles as the deliverable to install broadly.

## Operational limits to make explicit

- Advisory application locking and tree-relative IO do not make a concurrently modified filesystem an immutable snapshot; cameras/other writers must not mutate a source being ingested.
- Destination verification proves bytes read back in that run, not indefinite storage health or an absolute power-loss guarantee.
- Exact cloned filesystem identity and hardware bridges can limit identity strength. Fail closed when identity is unavailable; do not replace it with a display label.
- Existing archive/finals, Final Cut libraries and active Clipper queues need their own dependency/lease policies. Do not infer cleanup permission from lack of a known lease.
- Windows/Linux support, multi-host catalogs and remote mutation endpoints are separate deliveries.
