#!/usr/bin/env bash
#
# Pins the parts of nu9ve/core (or nu9ve/inner-core) that an app uses into <app>/vendor/nu9ve-core/
# (or vendor/nu9ve-inner-core/). The app lists them, one path per line, in vendor/nu9ve-core.paths
# (or vendor/nu9ve-inner-core.paths).
#
#   sync-core.sh [<commit>]        copy those paths from a commit of the repo (default: HEAD)
#   sync-core.sh --from <folder>   copy from a working tree to try a change; --check rejects it
#   sync-core.sh --check           the copy matches its manifest; warns if the repo moved on
#   sync-core.sh --strict          --check, and fails if the repo moved on
#
# Options: --repo <core|inner-core>  which repo (default: core)
#          --app <folder>     the app checkout (default: the git checkout around the current folder)
#          --source <folder>  the repo's checkout (default for core: $NU9VE_CORE_REPO, <app>/../core,
#                             ~/code/ai/core; for inner-core: $NU9VE_INNER_CORE_REPO, <app>/../inner-core,
#                             ~/code/ai/inner-core)
#
# --check needs no checkout of the repo, so a clean CI runner can use it.
# A core pin carries this script, so an app runs vendor/nu9ve-core/scripts/sync-core.sh, also with
# --repo inner-core: an inner-core pin does not carry it. Written for the bash 3.2 that ships with macOS.

set -Eeuo pipefail

manifest="MANIFEST.sha256"
source_file="SOURCE"
self_path="scripts/sync-core.sh"

die() {
    echo "sync-core: $*" >&2
    exit 1
}

usage() {
    sed -n '3,20p' "$0" | sed -E 's/^# ?//' >&2
    exit 64
}

mode="sync"
commit=""
from=""
app=""
repo="core"
source_repo=""
source_given=""
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
            source_given=1
            shift
            ;;
        --repo)
            [[ $# -ge 2 ]] || usage
            repo="$2"
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
case "${repo}" in
    core) env_repo="${NU9VE_CORE_REPO:-}" ;;
    inner-core) env_repo="${NU9VE_INNER_CORE_REPO:-}" ;;
    *) usage ;;
esac
label="nu9ve/${repo}"
dir_name="nu9ve-${repo}"
app="$(cd -- "${app}" && pwd)" || die "no app folder at ${app}"
vendor="${app}/vendor/${dir_name}"
paths_file="${app}/vendor/${dir_name}.paths"

# A checkout is the repo when its HEAD carries .nu9ve-repo with the repo's name. nu9ve/core is also
# recognized the old way, by carrying this script, so an app's own core/ is never taken for it.
is_repo() {
    [[ "$(git -C "$1" show "HEAD:.nu9ve-repo" 2>/dev/null | head -n 1)" == "${label}" ]] && return 0
    [[ "${repo}" == core ]] && git -C "$1" cat-file -e "HEAD:${self_path}" 2>/dev/null
}

# Without --source or the variable: a sibling checkout, then the suite's usual one.
if [[ -z "${source_given}" ]]; then
    source_repo="${env_repo}"
fi
if [[ -z "${source_repo}" ]]; then
    source_repo="${app}/../${repo}"
    for candidate in "${app}/../${repo}" "${HOME:-/nonexistent}/code/ai/${repo}"; do
        if is_repo "${candidate}"; then
            source_repo="${candidate}"
            break
        fi
    done
fi

sha256() {
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$@"
    else
        sha256sum "$@"
    fi
}

# The declared paths (plus this script, for a core pin), normalized and sorted.
wanted_paths() {
    [[ -f "${paths_file}" ]] || die "no ${paths_file}. List there the files or folders of ${label} this app uses, one per line."
    {
        sed -E -e 's/#.*//' -e 's/^[[:space:]]+//' -e 's/[[:space:]]+$//' -e 's#^\./##' -e 's#/+$##' "${paths_file}"
        if [[ "${repo}" == core ]]; then echo "${self_path}"; fi
    } | grep -v '^$' | LC_ALL=C sort -u
}

paths=()
staging=""
load_paths() {
    local p
    while IFS= read -r p; do
        case "${p}" in
            /* | .. | ../* | */.. | */../*) die "${p}: paths are relative to ${label} and stay inside it" ;;
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
    mktemp -d "${TMPDIR:-/tmp}/${dir_name}.XXXXXX"
}

# Writes the manifest and SOURCE into the staging folder and replaces the vendor folder with it.
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
        echo "# Copia fijada de ${label}. No se edita: el cambio va a ${label} y se vuelve a sincronizar."
        echo "repo=${label}"
        for f in "$@"; do echo "${f}"; done
        for p in "${paths[@]}"; do echo "path=${p}"; done
    } > "${staging}/${source_file}"

    case "${vendor}" in
        */vendor/nu9ve-core | */vendor/nu9ve-inner-core) ;;
        *) die "refusing to replace ${vendor}" ;;
    esac
    mkdir -p -- "${app}/vendor"
    rm -rf -- "${vendor}.new"
    cp -R -- "${staging}" "${vendor}.new"
    rm -rf -- "${vendor}"
    mv -- "${vendor}.new" "${vendor}"
}

sync_commit() {
    local variable="NU9VE_CORE_REPO"
    [[ "${repo}" == inner-core ]] && variable="NU9VE_INNER_CORE_REPO"
    source_available || die "no ${label} checkout at ${source_repo}. Use --source or ${variable}."
    local rev full date p
    rev="${commit:-HEAD}"
    full="$(git -C "${source_repo}" rev-parse --verify --quiet "${rev}^{commit}")" || die "${rev} is not a commit of ${source_repo}"
    date="$(git -C "${source_repo}" log -1 --format=%cs "${full}")"
    for p in "${paths[@]}"; do
        git -C "${source_repo}" cat-file -e "${full}:${p}" 2>/dev/null || die "${p} does not exist in ${label}@${full:0:12}"
    done
    staging="$(make_staging)"
    trap 'rm -rf -- "${staging:-}"' EXIT
    mkdir -p -- "${staging}/copy"
    # git archive applies core.autocrlf: without this, Git on Windows would pin CRLF bytes.
    git -C "${source_repo}" -c core.autocrlf=false -c core.eol=lf archive --format=tar "${full}" -- "${paths[@]}" | tar -x -C "${staging}/copy"
    publish "${staging}/copy" "commit=${full}" "date=${date}"
    echo "Pinned ${label}@${full:0:12} (${date}) into vendor/${dir_name}: ${#paths[@]} paths."
}

sync_from() {
    local src base p f
    src="$(cd -- "${from}" && pwd)" || die "no folder at ${from}"
    git -C "${src}" rev-parse --git-dir >/dev/null 2>&1 || die "${src} is not a ${label} checkout"
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

# True when a file that fails its manifest matches it once its CRs are gone. Hashes, not grep:
# Git for Windows' grep reports '\r' in LF files when its output goes to a pipe.
crlf_converted() {
    local line hash f actual stripped
    while IFS= read -r line; do
        hash="${line%% *}"
        f="${line:$((${#hash} + 2))}"
        [[ -f "${vendor}/${f}" ]] || continue
        actual="$(sha256 < "${vendor}/${f}")"
        [[ "${actual%% *}" == "${hash}" ]] && continue
        stripped="$(LC_ALL=C tr -d '\r' < "${vendor}/${f}" | sha256)"
        [[ "${stripped%% *}" == "${hash}" ]] && return 0
    done < "${vendor}/${manifest}"
    return 1
}

verify_integrity() {
    [[ -f "${vendor}/${manifest}" && -f "${vendor}/${source_file}" ]] || die "vendor/${dir_name} has no pinned copy. Run sync-core.sh."
    if grep -q '^dirty=' "${vendor}/${source_file}"; then
        die "vendor/${dir_name} came from a working tree (--from). Pin a commit: sync-core.sh <commit>."
    fi
    if ! grep -qx "repo=${label}" "${vendor}/${source_file}"; then
        die "vendor/${dir_name} is not a pin of ${label}. Run sync-core.sh."
    fi
    if ! (cd -- "${vendor}" && sha256 --check --quiet "${manifest}"); then
        # Git on Windows (core.autocrlf=true) checks text out with CRLF: same content, other bytes.
        if crlf_converted; then
            die "vendor/${dir_name} has CRLF line endings: Git converted it on checkout. Add 'vendor/${dir_name}/** -text' to the app's .gitattributes, then delete vendor/${dir_name} and restore it with 'git checkout -- vendor/${dir_name}'."
        fi
        die "vendor/${dir_name} differs from its manifest. Do not edit it: change ${label} and sync."
    fi
    # Every file must be declared, or an unreviewed file could slip in.
    local declared present
    declared="$(sed -E 's/^[0-9a-f]+ [ *]//' "${vendor}/${manifest}" | LC_ALL=C sort)"
    present="$(cd -- "${vendor}" && find . -type f ! -name "${manifest}" ! -name "${source_file}" | sed 's#^\./##' | LC_ALL=C sort)"
    if [[ "${declared}" != "${present}" ]]; then
        echo "sync-core: vendor/${dir_name} has files outside its manifest:" >&2
        diff <(echo "${declared}") <(echo "${present}") >&2 || true
        exit 1
    fi
    # The pin must cover exactly what the app declares.
    local pinned_paths
    pinned_paths="$(sed -n 's/^path=//p' "${vendor}/${source_file}")"
    if [[ "${pinned_paths}" != "$(printf '%s\n' "${paths[@]}")" ]]; then
        echo "sync-core: vendor/${dir_name}.paths changed after the last pin. Run sync-core.sh." >&2
        diff <(echo "${pinned_paths}") <(printf '%s\n' "${paths[@]}") >&2 || true
        exit 1
    fi
}

check_freshness() {
    local strict="$1" pinned
    pinned="$(pinned_commit)"
    if ! source_available; then
        if [[ "${strict}" == "strict" ]]; then
            die "--strict needs a ${label} checkout to compare against (--source or its variable)."
        fi
        echo "${label} pinned at ${pinned:0:12}; no checkout to compare against."
        return 0
    fi
    if ! git -C "${source_repo}" cat-file -e "${pinned}^{commit}" 2>/dev/null; then
        echo "sync-core: the pinned commit ${pinned:0:12} is not in ${source_repo}. Fetch it to compare." >&2
        [[ "${strict}" == "strict" ]] && exit 1
        return 0
    fi
    if git -C "${source_repo}" diff --quiet "${pinned}" HEAD -- "${paths[@]}"; then
        echo "${label} pinned at ${pinned:0:12} is current."
        return 0
    fi
    echo "sync-core: ${label} changed what this app uses after ${pinned:0:12}. Review and run sync-core.sh:" >&2
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
