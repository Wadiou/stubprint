# Development only: the Rust toolchain and a watcher that builds the server and starts it again
# whenever the Rust code changes. The source is mounted by compose.yaml, not copied.
FROM rust:1
ARG WATCHEXEC=2.8.0
RUN name="watchexec-${WATCHEXEC}-$(uname -m)-unknown-linux-musl" \
  && curl -fsSL "https://github.com/watchexec/watchexec/releases/download/v${WATCHEXEC}/${name}.tar.xz" \
  | tar -xJ -C /usr/local/bin --strip-components=1 "${name}/watchexec"
WORKDIR /work
CMD ["watchexec", "--restart", "--watch", "crates", "--watch", "Cargo.toml", "--watch", "Cargo.lock", "--exts", "rs,toml", "--", \
  "cargo", "run", "-p", "stubprint-server", "--", "--http", "0.0.0.0:8080", "--printer", "name=printer,listen=0.0.0.0:9100"]
