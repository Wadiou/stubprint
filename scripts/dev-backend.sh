#!/bin/sh
# Development only: runs the server and starts it again whenever the Rust code changes.
set -e
VERSION=2.8.0
NAME="watchexec-$VERSION-$(uname -m)-unknown-linux-musl"
if [ ! -x /tools/watchexec ]; then
  curl -fsSL "https://github.com/watchexec/watchexec/releases/download/v$VERSION/$NAME.tar.xz" \
    | tar -xJ -C /tools --strip-components=1 "$NAME/watchexec"
fi
exec /tools/watchexec --restart --watch crates --watch Cargo.toml --watch Cargo.lock --exts rs,toml \
  -- cargo run -p stubprint-server -- --http 0.0.0.0:8080 --printer name=printer,listen=0.0.0.0:9100
