#!/usr/bin/env bash
# Packs the built program, with its licence and the notices of what it holds, into
# dist/stubprint-<version>-<target>.tar.gz, or .zip for Windows. The release workflow runs it for
# every target after `cargo build --release --target <target> -p stubprint-server`.
#
#   scripts/package.sh x86_64-unknown-linux-gnu 0.4.0
set -euo pipefail

target="${1:?say the target, such as x86_64-unknown-linux-gnu}"
version="${2:?say the version, such as 0.4.0}"
cd "$(dirname "$0")/.."

name="stubprint-${version}-${target}"
case "$target" in
  *windows*) binary=stubprint.exe ;;
  *) binary=stubprint ;;
esac

built="target/${target}/release/${binary}"
[ -f "$built" ] || { echo "$built is not built" >&2; exit 1; }

rm -rf "dist/${name}" "dist/${name}.tar.gz" "dist/${name}.zip"
mkdir -p "dist/${name}"
cp "$built" LICENSE README.md THIRD-PARTY.md "dist/${name}/"

case "$target" in
  *windows*)
    if command -v 7z >/dev/null 2>&1; then
      (cd dist && 7z a -tzip -bso0 "${name}.zip" "${name}")
    else
      (cd dist && zip -qr "${name}.zip" "${name}")
    fi
    echo "dist/${name}.zip"
    ;;
  *)
    tar -czf "dist/${name}.tar.gz" -C dist "${name}"
    echo "dist/${name}.tar.gz"
    ;;
esac
rm -rf "dist/${name}"
