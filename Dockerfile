# syntax=docker/dockerfile:1.20.0@sha256:26147acbda4f14c5add9946e2fd2ed543fc402884fd75146bd342a7f6271dc1d

FROM rust:1.98.0-slim-bookworm@sha256:1469a27c125cb5a3aebfa4f4e4665d935b02fb72cc093b2c974b3d740e43f157 AS rust-builder
ARG KEYCAST_BUILD_REVISION=development
ENV KEYCAST_BUILD_REVISION=${KEYCAST_BUILD_REVISION}
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY api ./api
COPY core ./core
COPY signer ./signer
COPY database/migrations ./database/migrations
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked \
    && install -Dm755 target/release/keycast_api /out/keycast_api \
    && install -Dm755 target/release/keycast_signer /out/keycast_signer

FROM oven/bun:1.3.9@sha256:856da45d07aeb62eb38ea3e7f9e1794c0143a4ff63efb00e6c4491b627e2a521 AS web-builder
ARG KEYCAST_BUILD_REVISION=development
ENV KEYCAST_BUILD_REVISION=${KEYCAST_BUILD_REVISION}
WORKDIR /app
ENV CI=true NODE_ENV=production VITE_BUILD_MODE=production
ARG VITE_DOMAIN
ENV VITE_DOMAIN=${VITE_DOMAIN}
COPY web/package.json web/bun.lockb ./
RUN --mount=type=cache,target=/root/.bun/install/cache bun install --frozen-lockfile
COPY web ./
RUN bun run check && bun run build

FROM oven/bun:1.3.9@sha256:856da45d07aeb62eb38ea3e7f9e1794c0143a4ff63efb00e6c4491b627e2a521 AS web-runtime-deps
WORKDIR /app
COPY web/package.json web/bun.lockb ./
RUN bun install --production --frozen-lockfile

FROM debian:bookworm-slim@sha256:88200866dfff7ea7f5cbcb6ec7c8a701889efe6fe859fe64d6990e4b07ea4171 AS rust-runtime-base
ARG KEYCAST_VERSION=development
ARG KEYCAST_BUILD_REVISION=development
LABEL org.opencontainers.image.version=${KEYCAST_VERSION} \
      org.opencontainers.image.revision=${KEYCAST_BUILD_REVISION}
LABEL org.opencontainers.image.source="https://github.com/marmot-protocol/keycast"
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN groupadd --system --gid 10001 keycast \
    && useradd --system --uid 10001 --gid keycast --home-dir /nonexistent --shell /usr/sbin/nologin keycast \
    && install -d -o keycast -g keycast -m 0700 /app/database \
    && install -d -o keycast -g keycast -m 0750 /run/keycast
COPY --chown=root:root database/migrations /app/migrations
WORKDIR /app
ENV KEYCAST_DATABASE_PATH=/app/database/keycast-v2.db \
    KEYCAST_MIGRATIONS_PATH=/app/migrations \
    KEYCAST_SIGNER_SOCKET=/run/keycast/signer.sock
USER keycast:keycast

FROM rust-runtime-base AS api-runtime
COPY --chown=root:root --chmod=0555 --from=rust-builder /out/keycast_api /app/keycast_api
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=6s --start-period=10s --retries=3 CMD ["/app/keycast_api", "healthcheck"]
ENTRYPOINT ["/app/keycast_api"]

FROM rust-runtime-base AS signer-runtime
COPY --chown=root:root --chmod=0555 --from=rust-builder /out/keycast_signer /app/keycast_signer
ENV KEYCAST_ROOT_KEY_FILE=/run/secrets/keycast-root-key
HEALTHCHECK --interval=30s --timeout=6s --start-period=10s --retries=3 CMD ["/app/keycast_signer", "healthcheck"]
ENTRYPOINT ["/app/keycast_signer"]

FROM node:24-bookworm-slim@sha256:ba849c60be29959425b8734d57b8b4b7d56f98edd9504c9af091d5281095a71e AS web-runtime
ARG KEYCAST_VERSION=development
ARG KEYCAST_BUILD_REVISION=development
LABEL org.opencontainers.image.version=${KEYCAST_VERSION} \
      org.opencontainers.image.revision=${KEYCAST_BUILD_REVISION}
LABEL org.opencontainers.image.source="https://github.com/marmot-protocol/keycast"
RUN groupadd --system --gid 10001 keycast \
    && useradd --system --uid 10001 --gid keycast --home-dir /nonexistent --shell /usr/sbin/nologin keycast \
    && install -d -o root -g root -m 0755 /app
WORKDIR /app
COPY --chown=root:root --from=web-builder /app/build ./web
COPY --chown=root:root --from=web-builder /app/package.json ./package.json
COPY --chown=root:root --from=web-runtime-deps /app/node_modules ./node_modules
ENV NODE_ENV=production HOST=0.0.0.0 PORT=5173
USER keycast:keycast
EXPOSE 5173
HEALTHCHECK --interval=30s --timeout=6s --start-period=10s --retries=3 \
    CMD ["node", "-e", "fetch('http://127.0.0.1:5173/health').then(r=>{if(!r.ok)process.exit(1)}).catch(()=>process.exit(1))"]
ENTRYPOINT ["node", "/app/web/index.js"]
