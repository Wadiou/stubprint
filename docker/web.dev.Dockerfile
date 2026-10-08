# Development only: Node and pnpm, and a watcher that installs the packages again whenever they
# change. Vite reloads the page as it is edited. The source is mounted by compose.yaml.
FROM node:24
ARG WATCHEXEC=2.8.0
RUN name="watchexec-${WATCHEXEC}-$(uname -m)-unknown-linux-musl" \
  && curl -fsSL "https://github.com/watchexec/watchexec/releases/download/v${WATCHEXEC}/${name}.tar.xz" \
  | tar -xJ -C /usr/local/bin --strip-components=1 "${name}/watchexec" \
  && corepack enable
WORKDIR /work
CMD ["watchexec", "--restart", "--workdir", "/work/web", "--watch", "/work/web/package.json", "--watch", "/work/web/pnpm-lock.yaml", "--", \
  "pnpm install --frozen-lockfile && pnpm dev --host 0.0.0.0"]
