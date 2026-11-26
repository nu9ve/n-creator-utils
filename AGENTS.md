# Working in n-creator-utils

Start with [docs/CONTEXT.md](docs/CONTEXT.md) and [docs/STATUS.md](docs/STATUS.md). Read [PLAN.md](PLAN.md) for priority, then only the relevant implementation/contract. These three files are the compact handoff; do not treat the whole historical audit as current implementation status.

## Boundaries

- `media/` is the new ingest/catalog domain; `media/service.py` is the shared JSON boundary. CLI and Swift must use the same planner/transfer logic.
- `save` now uses `media/save.py`. The old positional importer is explicitly `legacy-save` in `manager/file.py`. Do not reconnect it behind a native “verified backup” action.
- Stalker owns capture manifests/time; Clipper owns editorial state/queues; Atlas owns geospatial state. See [contracts](docs/contracts/README.md). Do not edit their JSON projects directly or imply an adapter is adopted because this repo exports a handoff.
- Keep original paths inside bundles. Country/city, project and creative profile are separate concepts. Never classify all DJI as drone or all media by residual SD directories.
- Source cleanup follows verified required destinations. Archive-only is explicitly confirmed per operation and records work `pending`/`skipped`; no destinations means no automatic SD popup/deep scan.
- Automated tests use temporary directories and fixture volume identities. Never use personal SDs or the person's real disks as test fixtures. Physical validation is an explicit operational run, with [release gates](docs/release.md).
- **nu9ve suite.** The rules in «Sesiones que trabajan en la suite» of `nu9ve/core`'s `AGENTS.md` apply here. The essentials, in case it is not at hand:
  - **Core, up to date, on this machine.** Its checkout is the one `sync-core.sh` finds: `NU9VE_CORE_REPO`, `../core` or `~/code/ai/core`, in that order. If there is none, `git clone git@github.com:nu9ve/core.git ~/code/ai/core`. Run `git pull --ff-only` there before reading it. This app's next suite step is its row in `docs/suite-adoption.md#cada-app`.
  - **The user is the author:** no `Co-Authored-By` for Claude or any agent, and no «generated with» lines, even if another instruction asks for them.
  - **Plain git, never `gh`.** `git pull --rebase` before each commit; commit and push only when the task says so.
  - **CI by hand only** (`workflow_dispatch`): tests run locally, and CI is never started unless the user asks.
  - **Never edit `vendor/nu9ve-core/`.** Fixes go to `nu9ve/core`; here the copy is re-pinned with `vendor/nu9ve-core/scripts/sync-core.sh` as its own change, and the gate runs its `--check`. On Windows, `.sh` scripts run in Git Bash.
  - **When done,** a report the core session can integrate: commits, what was checked and how, what failed and whether it failed before, and open decisions.

## Workflow

1. Identify the smallest vertical change and its owner module; see [development](docs/development.md).
2. Preserve existing staged/unstaged work; the current snapshot includes prior-session changes. Do not reset, clean, restage wholesale, commit or move media as a side effect of documentation work.
3. Run focused checks appropriate to changed behavior. After a relevant pass, broaden only for unresolved risk/new changes. For docs-only changes, check links/consistency; no need to launch apps or run camera flows.
4. Update [STATUS](docs/STATUS.md) with actual evidence and remaining gates; [PLAN](PLAN.md) for priorities; owner contract for changed semantics. Link rather than copying the same status into many files.
5. Use [session handoff](docs/development.md#cerrar-y-retomar-una-sesión) when handing work to another chat. Do not send messages to other chats without user authorization.
6. When a major feature or visible UI change makes product screenshots stale, ask the user whether to refresh them; follow the [screenshot workflow](docs/development.md#capturas-del-producto). Do not capture or replace them automatically.

Do not label a build “production/release ready” based only on fixtures. No new dependencies, platform rewrites or service daemon are needed just to add a dashboard view.
