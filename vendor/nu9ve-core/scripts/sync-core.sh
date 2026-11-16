#!/usr/bin/env bash
#
# Pins the parts of nu9ve/core that an app uses into <app>/vendor/nu9ve-core/.
# The app lists them, one path per line, in vendor/nu9ve-core.paths.
#
#   sync-core.sh [<commit>]        copy those paths from a commit of nu9ve/core (default: HEAD)
#   sync-core.sh --from <folder>   copy from a working tree to try a change; --check rejects it
#   sync-core.sh --check           the copy matches its manifest; warns if nu9ve/core moved on
#   sync-core.sh --strict          --check, and fails if nu9ve/core moved on
#
# Options: --app <folder>     the app checkout (default: the git checkout around the current folder)
#          --source <folder>  the nu9ve/core checkout (default: $NU9VE_CORE_REPO, then <app>/../core)
#
# --check needs no nu9ve/core checkout, so a clean CI runner can use it.
# The script copies itself into every pin, so an app runs vendor/nu9ve-core/scripts/sync-core.sh.
# Written for the bash 3.2 that ships with macOS.

set -Eeuo pipefail

manifest="MANIFEST.sha256"
source_file="SOURCE"
self_path="scripts/sync-core.sh"

die() {
    echo "sync-core: $*" >&2
    exit 1
}

usage() {
    sed -n '3,16p' "$0" | sed -E 's/^# ?//' >&2
    exit 64
}

mode="sync"
commit=""
from=""
app=""
source_repo="${NU9VE_CORE_REPO:-}"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --check) mode="check" ;;
        --strict) mode="strict" ;;
        --from)
            [[ $# -ge 2 ]] || usage
            mode="from"
            from="$2"
            shift
            ;;
        --app)
            [[ $# -ge 2 ]] || usage
            app="$2"
            shift
            ;;
        --source)
            [[ $# -ge 2 ]] || usage
            source_repo="$2"
            shift
            ;;
        -*) usage ;;
        *)
            [[ -z "${commit}" ]] || usage
            commit="$1"
            ;;
    esac
    shift
done
[[ -z "${commit}" || "${mode}" == "sync" ]] || usage

if [[ -z "${app}" ]]; then
    app="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
fi
app="$(cd -- "${app}" && pwd)" || die "no app folder at ${app}"
vendor="${app}/vendor/nu9ve-core"
paths_file="${app}/vendor/nu9ve-core.paths"
[[ -n "${source_repo}" ]] || source_repo="${app}/../core"

sha256() {
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$@"
    else
        sha256sum "$@"
    fi
}

# The declared paths plus the script itself, normalized and sorted.
wanted_paths() {
    [[ -f "${paths_file}" ]] || die "no ${paths_file}. List there the files or folders of nu9ve/core this app uses, one per line."
    {
        sed -E -e 's/#.*//' -e 's/^[[:space:]]+//' -e 's/[[:space:]]+$//' -e 's#^\./##' -e 's#/+$##' "${paths_file}"
        echo "${self_path}"
    } | grep -v '^$' | LC_ALL=C sort -u
}

paths=()
staging=""
load_paths() {
    local p
    while IFS= read -r p; do
        case "${p}" in
            /* | .. | ../* | */.. | */../*) die "${p}: paths are relative to nu9ve/core and stay inside it" ;;
        esac
        paths+=("${p}")
    done < <(wanted_paths)
}

source_available() {
    git -C "${source_repo}" rev-parse --git-dir >/dev/null 2>&1
}

pinned_commit() {
    sed -n 's/^commit=//p' "${vendor}/${source_file}"
}

make_staging() {
    mktemp -d "${TMPDIR:-/tmp}/nu9ve-core.XXXXXX"
}

# Writes the manifest and SOURCE into the staging folder and replaces vendor/nu9ve-core with it.
publish() {
    local staging="$1"
    shift
    local f p
    (
        cd -- "${staging}"
        find . -type f | sed 's#^\./##' | LC_ALL=C sort | while IFS= read -r f; do
            sha256 "${f}"
        done
    ) > "${staging}/../${manifest}.$$"
    mv -- "${staging}/../${manifest}.$$" "${staging}/${manifest}"
    {
        echo "# Copia fijada de nu9ve/core. No se edita: el cambio va a nu9ve/core y se vuelve a sincronizar."
        echo "repo=nu9ve/core"
        for f in "$@"; do echo "${f}"; done
        for p in "${paths[@]}"; do echo "path=${p}"; done
    } > "${staging}/${source_file}"

    case "${vendor}" in
        */vendor/nu9ve-core) ;;
        *) die "refusing to replace ${vendor}" ;;
    esac
    mkdir -p -- "${app}/vendor"
    rm -rf -- "${vendor}.new"
    cp -R -- "${staging}" "${vendor}.new"
    rm -rf -- "${vendor}"
    mv -- "${vendor}.new" "${vendor}"
}

sync_commit() {
    source_available || die "no nu9ve/core checkout at ${source_repo}. Use --source or NU9VE_CORE_REPO."
    local rev full date p
    rev="${commit:-HEAD}"
    full="$(git -C "${source_repo}" rev-parse --verify --quiet "${rev}^{commit}")" || die "${rev} is not a commit of ${source_repo}"
    date="$(git -C "${source_repo}" log -1 --format=%cs "${full}")"
    for p in "${paths[@]}"; do
        git -C "${source_repo}" cat-file -e "${full}:${p}" 2>/dev/null || die "${p} does not exist in nu9ve/core@${full:0:12}"
    done
    staging="$(make_staging)"
    trap 'rm -rf -- "${staging:-}"' EXIT
    mkdir -p -- "${staging}/copy"
    git -C "${source_repo}" archive --format=tar "${full}" -- "${paths[@]}" | tar -x -C "${staging}/copy"
    publish "${staging}/copy" "commit=${full}" "date=${date}"
    echo "Pinned nu9ve/core@${full:0:12} (${date}) into vendor/nu9ve-core: ${#paths[@]} paths."
}

sync_from() {
    local src base p f
    src="$(cd -- "${from}" && pwd)" || die "no folder at ${from}"
    git -C "${src}" rev-parse --git-dir >/dev/null 2>&1 || die "${src} is not a nu9ve/core checkout"
    base="$(git -C "${src}" rev-parse --verify --quiet HEAD || echo none)"
    staging="$(make_staging)"
    trap 'rm -rf -- "${staging:-}"' EXIT
    mkdir -p -- "${staging}/copy"
    for p in "${paths[@]}"; do
        [[ -e "${src}/${p}" ]] || die "${p} does not exist in ${src}"
        # Tracked and new files, without what .gitignore leaves out (build folders, caches).
        while IFS= read -r f; do
            [[ -f "${src}/${f}" ]] || continue
            mkdir -p -- "${staging}/copy/$(dirname -- "${f}")"
            cp -p -- "${src}/${f}" "${staging}/copy/${f}"
        done < <(git -C "${src}" ls-files --cached --others --exclude-standard -- "${p}")
    done
    publish "${staging}/copy" "commit=${base}" "dirty=working-tree"
    echo "Copied ${#paths[@]} paths from a working tree. This copy is dirty: --check fails until you pin a commit."
}

verify_integrity() {
    [[ -f "${vendor}/${manifest}" && -f "${vendor}/${source_file}" ]] || die "vendor/nu9ve-core has no pinned copy. Run sync-core.sh."
    if grep -q '^dirty=' "${vendor}/${source_file}"; then
        die "vendor/nu9ve-core came from a working tree (--from). Pin a commit: sync-core.sh <commit>."
    fi
    (cd -- "${vendor}" && sha256 --check --quiet "${manifest}") ||
        die "vendor/nu9ve-core differs from its manifest. Do not edit it: change nu9ve/core and sync."
    # Every file must be declared, or an unreviewed file could slip in.
    local declared present
    declared="$(sed -E 's/^[0-9a-f]+ [ *]//' "${vendor}/${manifest}" | LC_ALL=C sort)"
    present="$(cd -- "${vendor}" && find . -type f ! -name "${manifest}" ! -name "${source_file}" | sed 's#^\./##' | LC_ALL=C sort)"
    if [[ "${declared}" != "${present}" ]]; then
        echo "sync-core: vendor/nu9ve-core has files outside its manifest:" >&2
        diff <(echo "${declared}") <(echo "${present}") >&2 || true
        exit 1
    fi
    # The pin must cover exactly what the app declares.
    local pinned_paths
    pinned_paths="$(sed -n 's/^path=//p' "${vendor}/${source_file}")"
    if [[ "${pinned_paths}" != "$(printf '%s\n' "${paths[@]}")" ]]; then
        echo "sync-core: vendor/nu9ve-core.paths changed after the last pin. Run sync-core.sh." >&2
        diff <(echo "${pinned_paths}") <(printf '%s\n' "${paths[@]}") >&2 || true
        exit 1
    fi
}

check_freshness() {
    local strict="$1" pinned
    pinned="$(pinned_commit)"
    if ! source_available; then
        if [[ "${strict}" == "strict" ]]; then
            die "--strict needs a nu9ve/core checkout to compare against (--source or NU9VE_CORE_REPO)."
        fi
        echo "nu9ve/core pinned at ${pinned:0:12}; no checkout to compare against."
        return 0
    fi
    if ! git -C "${source_repo}" cat-file -e "${pinned}^{commit}" 2>/dev/null; then
        echo "sync-core: the pinned commit ${pinned:0:12} is not in ${source_repo}. Fetch it to compare." >&2
        [[ "${strict}" == "strict" ]] && exit 1
        return 0
    fi
    if git -C "${source_repo}" diff --quiet "${pinned}" HEAD -- "${paths[@]}"; then
        echo "nu9ve/core pinned at ${pinned:0:12} is current."
        return 0
    fi
    echo "sync-core: nu9ve/core changed what this app uses after ${pinned:0:12}. Review and run sync-core.sh:" >&2
    git -C "${source_repo}" diff --stat "${pinned}" HEAD -- "${paths[@]}" >&2
    [[ "${strict}" == "strict" ]] && exit 1
    return 0
}

load_paths
case "${mode}" in
    sync)
        sync_commit
        verify_integrity
        ;;
    from) sync_from ;;
    check)
        verify_integrity
        check_freshness warn
        ;;
    strict)
        verify_integrity
        check_freshness strict
        ;;
esac
