#!/bin/sh
# Installs Stubprint @VERSION@ for macOS or Linux: it downloads the archive of this computer from the
# release, checks it against SHA256SUMS and puts `stubprint` in a folder.
#
#   curl -fsSL https://stubprint.dev/install.sh | sh
#   curl -fsSL https://stubprint.dev/install.sh | sh -s -- --dir /usr/local/bin
#
# --dir <folder>   where to put it (STUBPRINT_INSTALL_DIR), by default ~/.local/bin
# STUBPRINT_DOWNLOAD_URL   where the files of the release are, for a mirror or a test
#
# The release attaches this script with the version filled in, so it installs that version.
set -eu

version="@VERSION@"
repo="Wadiou/stubprint"
base="${STUBPRINT_DOWNLOAD_URL:-https://github.com/${repo}/releases/download/v${version}}"
dir="${STUBPRINT_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
  echo "error: $*" >&2
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
    --dir)
      [ $# -ge 2 ] || fail "--dir needs a folder"
      dir="$2"
      shift 2
      ;;
    -h | --help)
      echo "Installs Stubprint ${version}."
      echo "  --dir <folder>           where to put it (STUBPRINT_INSTALL_DIR), by default ~/.local/bin"
      echo "  STUBPRINT_DOWNLOAD_URL   where the files of the release are, for a mirror or a test"
      exit 0
      ;;
    *) fail "unknown option $1 (see --help)" ;;
  esac
done

case "$version" in
  @*) fail "this is the template in the repository: use the installer attached to a release" ;;
esac

case "$(uname -s)" in
  Linux) system=unknown-linux-gnu ;;
  Darwin) system=apple-darwin ;;
  *) fail "there is no build for $(uname -s): see https://github.com/${repo}/releases" ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) machine=x86_64 ;;
  aarch64 | arm64) machine=aarch64 ;;
  *) fail "there is no build for $(uname -m): see https://github.com/${repo}/releases" ;;
esac

# A shell started under Rosetta says x86_64 on an Apple chip, which runs the native build better.
if [ "$system" = apple-darwin ] && [ "$machine" = x86_64 ] &&
  [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = 1 ]; then
  machine=aarch64
fi

archive="stubprint-${version}-${machine}-${system}.tar.gz"

if command -v curl >/dev/null 2>&1; then
  download() { curl -fsSL -o "$2" "$1"; }
elif command -v wget >/dev/null 2>&1; then
  download() { wget -qO "$2" "$1"; }
else
  fail "curl or wget is needed"
fi

if command -v sha256sum >/dev/null 2>&1; then
  hash_of() { sha256sum "$1" | cut -d ' ' -f 1; }
elif command -v shasum >/dev/null 2>&1; then
  hash_of() { shasum -a 256 "$1" | cut -d ' ' -f 1; }
else
  fail "sha256sum or shasum is needed to check the download"
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "Downloading Stubprint ${version} for ${machine}-${system}"
download "${base}/${archive}" "${work}/${archive}" || fail "cannot download ${base}/${archive}"
download "${base}/SHA256SUMS" "${work}/SHA256SUMS" || fail "cannot download ${base}/SHA256SUMS"

expected="$(grep "  ${archive}\$" "${work}/SHA256SUMS" | cut -d ' ' -f 1)"
[ -n "$expected" ] || fail "SHA256SUMS has no line for ${archive}"
[ "$(hash_of "${work}/${archive}")" = "$expected" ] || fail "the download does not match its checksum"

tar -xzf "${work}/${archive}" -C "$work"
mkdir -p "$dir"
cp "${work}/stubprint-${version}-${machine}-${system}/stubprint" "${dir}/stubprint.new"
chmod 755 "${dir}/stubprint.new"
mv "${dir}/stubprint.new" "${dir}/stubprint"

echo "Installed ${dir}/stubprint"
case ":${PATH}:" in
  *":${dir}:"*) ;;
  *) echo "${dir} is not in your PATH: add it, or run ${dir}/stubprint" ;;
esac
