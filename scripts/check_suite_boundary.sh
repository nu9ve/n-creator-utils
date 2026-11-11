#!/usr/bin/env bash
# The suite's pieces never depend on nüün (docs/suite-split.md §1): a `nu9ve-*` crate or a `Nu9ve.*` project
# that names something of nüün in code or in a manifest fails here. Comments may cite where a piece came from.
# The day these pieces move to nu9ve/suite, moving them is copying folders, not untangling them.
#
# Usage: scripts/check_suite_boundary.sh   (macOS, Linux or Git Bash on Windows; no dependencies)
set -euo pipefail
cd "$(dirname "$0")/.."

pieces=(engine/crates/nu9ve-* native/windows/Nu9ve.Core native/windows/Nu9ve.WinUI)
failed=0
checked=0

while IFS= read -r -d '' file; do
  checked=$((checked + 1))
  # Drop whole-line comments (//, ///, //!, # in TOML, <!-- --> on one line in XML), then look for nüün names.
  hits=$(sed -E -e 's/^[[:space:]]*(\/\/|#).*$//' -e 's/<!--.*-->//g' "$file" \
    | grep -nE 'nuun|Nuun|NUUN' || true)
  if [ -n "$hits" ]; then
    failed=1
    while IFS= read -r line; do
      echo "$file:$line"
    done <<< "$hits"
  fi
done < <(find "${pieces[@]}" -type f \( -name '*.rs' -o -name 'Cargo.toml' -o -name '*.cs' -o -name '*.csproj' \
  -o -name '*.xaml' -o -name '*.props' \) -not -path '*/bin/*' -not -path '*/obj/*' -not -path '*/target/*' -print0)

if [ "$failed" -ne 0 ]; then
  echo "suite boundary: a shared piece names nüün (see docs/suite-split.md §1)" >&2
  exit 1
fi
echo "suite boundary: ok ($checked files in ${#pieces[@]} shared pieces)"
