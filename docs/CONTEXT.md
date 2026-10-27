# Compact project context

Snapshot: 2026-10-26, America/Hermosillo. Start here, then [STATUS](STATUS.md) and [PLAN](../PLAN.md). This is a handoff, not a claim of release readiness.

## Product and user workflow

n-creator-utils manages creative media: SD ingest, verified archive, working copies, discovery/catalog and retrieval. macOS is first. Stalker captures; Clipper edits/syncs/exports; Final Cut finishes manual edits; Atlas consumes drone images. Downloads/stems remain separate utility arms; the old embedded Clipper was removed.

Typical video flow: SD → MEDIA archive + ESSD working raw → Clipper → Final Cut → finals → published → MEDIA finals. Audio normally needs MEDIA + SSD1. Projects/profiles can be mixed on one card; project/country/city/profile are independent fields. Preserve selection order, sidecars and complete Stalker bundles.

Existing disk layout should remain useful: ESSD `001_proyectos/e64_raw`, `001_proyectos/e64`, `002_finales`; MEDIA502 `001_raw`, `002_finales`, `003_curado`. Do not reorganize old TBs to introduce the catalog. Named layout (plan schema 2, [layout.py](../media/layout.py), decided at plan time and part of the digest) restores the historical names `YYMONDD-HHMMSS_MODEL_MM-SS_COUNTRY-PROJECT[].EXT` and MEDIA type/year/month folders; Sony XML goes to `metadata/sony/`, camera indexes/thumbnails keep their tree under `metadata/sony/support/`. Work copies are flat in `<profile>_raw/<project>/` with only editable media and whole Stalker bundles, so a folder lists in capture order. Copies land as `.name.ncpartial`, are verified, then renamed without replacing anything; identical existing files are adopted, different ones block the plan. Schema-1 plans keep per-job containers. Stalker bundle folders take the same dated name (contents untouched); `media relayout` renames earlier copies in place. Contract: [naming v1](contracts/media-naming-v1.md). Second-replica policy is a pending decision: [replication policy](features/replication-policy.md).

## Product decisions to preserve

- `save` should finish with the SD free for new projects. Copy → verify all effective required destinations → journaled cleanup. Copy-only is optional/diagnostic, not the intended default for a card.
- If MEDIA exists but ESSD/SSD1 is absent: explicitly confirm archive-only for that operation; record work pending or deliberately skipped. Later copy from archive, without the old SD. Do not silently make this exception the profile default.
- No registered archive/work destination online: SD insertion is silent, with no deep automatic scan. Manual inspect is still allowed.
- A completed backup, work readiness, publication and source readiness are different states. Never infer two replicas from two folders on one physical disk.
- Sony (`sony_clips_then_format`): after verification the engine removes clips, thumbnails and DCIM stills from the card but never its indexes (`AVF_INFO`, `PRIVATE/SONY`, `M4ROOT` databases, AVCHD index); the job ends `camera_action_required` until the camera format, confirmed by `confirm-cleanup` (indexes must have been rewritten). DJI drone/360/action ambiguity requires an explicit category. No persistent device/model registry yet.
- Stalker must be completed; originals/raw microphone/events stay together. Work with active recording must not copy an incomplete session. Archive does not rewrite Clipper references.
- No silent media staging on the internal drive. Local catalog/journal metadata is expected; local media destinations require explicit registration/choice.

## Code map

```text
nu9ve.py → media/save.py (human save) / media/cli.py (JSON CLI/RPC/MCP)
media/service.py → planner + volumes + transfer + catalog + library
media/common.py → safe tree-relative IO, hashes, errors, JSON
native/MediaMenu → Swift UI / Process JSON bridge / FSEvents
manager/file.py → legacy-save + existing downloads/stems
```

Use [modules](modules.md), [operations](operations.md) and [development](development.md) for details. Contracts are indexed [here](contracts/README.md); avoid new direct writes to an editor's JSON.

## Current working state

Current base is `5a60349`; this session adds native identity/menus/themes, thumbnail grid, compact importer and per-unit editing-copy selection on top (see [STATUS](STATUS.md)). The app is installed as **n creator utils**; operation/installation instructions are in [operations](operations.md). `.vscode/` is unrelated. Automated tests never use MEDIA/ESSD/SDs; any live backup is an explicit, user-requested operational run with a reviewed plan.

Next: remaining review items, the first small physical run and real progress/notifications, per [PLAN](../PLAN.md). Exact tests, artifacts and known limitations are in [STATUS](STATUS.md).
