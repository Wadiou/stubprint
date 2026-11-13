#!/usr/bin/env bash
# Draws src/assets/receipt.png: the cafe sample as the default printer prints it, which is what the
# command on the page does. Needs ImageMagick, and a stubprint (STUBPRINT, else the debug build of
# the workspace).
#
# Run: pnpm receipt
set -euo pipefail
cd "$(dirname "$0")/.."

bin="${STUBPRINT:-../target/debug/stubprint}"
port=18081
stubprint=("$bin" --server "http://127.0.0.1:$port")
tmp="$(mktemp -d)"

"$bin" --ephemeral --http "127.0.0.1:$port" --printer name=default,listen=127.0.0.1:0 >/dev/null 2>&1 &
server=$!
trap 'kill "$server" 2>/dev/null; rm -rf "$tmp"' EXIT

for _ in $(seq 50); do "${stubprint[@]}" info >/dev/null 2>&1 && break; sleep 0.1; done

"${stubprint[@]}" print --sample cafe
"${stubprint[@]}" wait --after 0 --timeout 20 --id >/dev/null
sleep 1
"${stubprint[@]}" receipt show 1 --form picture --scale 2 -o "$tmp/receipt.png"

# The paper goes on after the last line, so the picture ends a little under it.
bottom=$(convert "$tmp/receipt.png" -format '%@' info: | awk -F'[x+]' '{print $2 + $4}')
width=$(identify -format '%w' "$tmp/receipt.png")
convert "$tmp/receipt.png" -crop "${width}x$((bottom + 48))+0+0" +repage -strip src/assets/receipt.png
identify src/assets/receipt.png
