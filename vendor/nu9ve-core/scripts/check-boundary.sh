#!/usr/bin/env bash
#
# The suite's boundary: the shared code knows no app.
#
#   1. Swift under swift/Sources imports only Apple's frameworks and the package's own
#      targets. An app's module (FractosCore, NuunKit…) is rejected without naming it here.
#   2. No code file under rust/, swift/ or windows/ names an app of the suite, not even in a
#      comment. Apps pass their names as parameters (ShellWindowHost(frameAutosavePrefix:),
#      Storage.Configure).
#
#   check-boundary.sh                 both rules on nu9ve/core
#   check-boundary.sh <folder>...     rule 2 on an app's own suite pieces that have not moved yet
#                                     (nüün: vendor/nu9ve-core/scripts/check-boundary.sh engine/crates/nu9ve-*),
#                                     so moving them later is copying folders, not untangling them.
#
# Docs and READMEs may name apps: they say who uses what. Written for the bash 3.2 of macOS;
# on Windows it runs in Git Bash.

set -Eeuo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# Bash 3.2 (macOS) with set -u rejects an empty array, so the arguments are counted, never the array.
if [[ $# -eq 0 ]]; then
    cd -- "${root}"
    folders=(rust swift windows)
else
    folders=("$@")
fi
for folder in "${folders[@]}"; do
    [[ -d "${folder}" ]] || { echo "check-boundary: no folder at ${folder}" >&2; exit 2; }
done
failed=0

fail() {
    echo "check-boundary: $*" >&2
    failed=1
}

# Apple's modules the shared Swift may use. Add one here when a piece needs it.
system_modules=" Foundation Darwin AppKit SwiftUI Combine Observation CoreGraphics CoreText QuartzCore os OSLog Dispatch Synchronization Vision CoreVideo AVFoundation CoreMIDI Testing XCTest "

if [[ $# -eq 0 && -d swift/Sources ]]; then
    own_modules=" "
    for dir in swift/Sources/*/; do
        own_modules="${own_modules}$(basename -- "${dir}") "
    done
    while IFS= read -r hit; do
        [[ -n "${hit}" ]] || continue
        module="$(echo "${hit#*:*:}" | sed -E 's/^[[:space:]]*(@[A-Za-z_]+[[:space:]]+)*((public|internal|package|private|fileprivate)[[:space:]]+)?import[[:space:]]+((class|struct|enum|protocol|func|var|let|typealias)[[:space:]]+)?([A-Za-z0-9_]+).*/\6/')"
        case "${system_modules}${own_modules}" in
            *" ${module} "*) ;;
            *) fail "${hit%%:*}: imports ${module}, which is not Apple's nor this package's. Shared code knows no app." ;;
        esac
    done < <(grep -rnE '^[[:space:]]*(@[A-Za-z_]+[[:space:]]+)*((public|internal|package|private|fileprivate)[[:space:]]+)?import[[:space:]]' swift/Sources swift/Tests --include='*.swift' || true)
fi

# The suite's apps, as their code and bundles name them.
apps='fractos|nuun|nüün|n-creator-utils|nove|curado|kyu|soundbender|soundshop|neuf|stalker|clipper'
code_files=()
while IFS= read -r f; do
    code_files+=("${f}")
done < <(find "${folders[@]}" -type f \( -name '*.swift' -o -name '*.rs' -o -name '*.cs' -o -name '*.xaml' -o -name '*.csproj' -o -name 'Cargo.toml' \) -not -path '*/.build/*' -not -path '*/target/*' -not -path '*/bin/*' -not -path '*/obj/*' 2>/dev/null)
if [[ ${#code_files[@]} -gt 0 ]]; then
    if grep -nHiE "(^|[^[:alnum:]_])(${apps})([^[:alnum:]_]|\$)" "${code_files[@]}" >&2; then
        fail "the lines above name an app. Take the name as a parameter instead."
    fi
fi

if [[ "${failed}" -ne 0 ]]; then
    exit 1
fi
echo "Boundary holds: ${#code_files[@]} code files name no app."
