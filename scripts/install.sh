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
# -e stops at the first command that fails, -u at a variable that was never set.
set -eu

version="@VERSION@"
repo="Wadiou/stubprint"
base="${STUBPRINT_DOWNLOAD_URL:-https://github.com/${repo}/releases/download/v${version}}"
dir="${STUBPRINT_INSTALL_DIR:-$HOME/.local/bin}"

# Colours for a terminal, and none for a pipe or when NO_COLOR is set.
# -t 1: the output is a terminal, and not a pipe or a file.
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  bold="$(printf '\033[1m')" dim="$(printf '\033[2m')" cyan="$(printf '\033[36m')"
  green="$(printf '\033[32m')" yellow="$(printf '\033[33m')" red="$(printf '\033[31m')" reset="$(printf '\033[0m')"
else
  bold="" dim="" cyan="" green="" yellow="" red="" reset=""
fi

fail() {
  echo "${red}${bold}error${reset}: $*" >&2
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

# The static build is for systems with musl, such as Alpine, and for glibc older than the one the
# other Linux build needs (2.35).
if [ "$system" = unknown-linux-gnu ]; then
  libc="$(ldd --version 2>&1 | head -n 1)"
  case "$libc" in
    *[Mm]usl*) system=unknown-linux-musl ;;
    *)
      # The number after the dot of the glibc version, or nothing when it cannot be read.
      minor="$(echo "$libc" | grep -o '[0-9][0-9]*\.[0-9][0-9]*$' | cut -d . -f 2)"
      [ -n "$minor" ] && [ "$minor" -lt 35 ] && system=unknown-linux-musl
      ;;
  esac
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

# The trap deletes the folder when the script ends, whether it worked or not.
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "${cyan}==>${reset} ${bold}Downloading Stubprint ${version}${reset} ${dim}for ${machine}-${system}${reset}"
download "${base}/${archive}" "${work}/${archive}" || fail "cannot download ${base}/${archive}"
download "${base}/SHA256SUMS" "${work}/SHA256SUMS" || fail "cannot download ${base}/SHA256SUMS"

# Two spaces, as sha256sum writes them, and `$` for the end of the line, so "a.tar.gz" cannot
# match a line of "a.tar.gz.sig".
expected="$(grep "  ${archive}\$" "${work}/SHA256SUMS" | cut -d ' ' -f 1)"
[ -n "$expected" ] || fail "SHA256SUMS has no line for ${archive}"
[ "$(hash_of "${work}/${archive}")" = "$expected" ] || fail "the download does not match its checksum"

tar -xzf "${work}/${archive}" -C "$work"
mkdir -p "$dir"
# Copy beside the final name and rename: a rename is one step, so a program that is running (as in
# `stubprint update`) is never half replaced.
cp "${work}/stubprint-${version}-${machine}-${system}/stubprint" "${dir}/stubprint.new"
chmod 755 "${dir}/stubprint.new"
mv "${dir}/stubprint.new" "${dir}/stubprint"

echo "${green}${bold}✓${reset} ${bold}Installed${reset} ${dir}/stubprint"
# The colons around both make the pattern match a whole entry of the PATH and not part of one.
case ":${PATH}:" in
  *":${dir}:"*)
    echo
    echo "Next, run ${cyan}${bold}stubprint${reset}"
    ;;
  *)
    echo "${yellow}!${reset} ${dir} is not in your PATH: add it, or run ${bold}${dir}/stubprint${reset}"
    ;;
esac
