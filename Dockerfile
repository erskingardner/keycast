# syntax=docker/dockerfile:1.7

# Build stage for Rust services
FROM rust:1.92.0-slim-bookworm@sha256:f1f73538ebe623fd3673a35aff3df358ae1084c64c55646516e5b17b321b6c9b AS rust-builder
WORKDIR /app
COPY ./api ./api
COPY ./signer ./signer
COPY ./core ./core
COPY ./Cargo.* .
COPY ./Cargo.lock .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked \
    && mkdir -p /app/bin \
    && cp /app/target/release/keycast_api /app/bin/keycast_api \
    && cp /app/target/release/keycast_signer /app/bin/keycast_signer \
    && cp /app/target/release/signer_daemon /app/bin/signer_daemon

# Build stage for Bun frontend
FROM oven/bun:1.3.9@sha256:856da45d07aeb62eb38ea3e7f9e1794c0143a4ff63efb00e6c4491b627e2a521 AS web-builder

# Install build essentials for native modules
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    python3 \
    python-is-python3 \
    make \
    g++ \
    && rm -rf /var/lib/apt/lists/*

ENV NODE_OPTIONS="--max-old-space-size=2048"
ENV CI=true
ENV NODE_ENV=production
ENV VITE_BUILD_MODE=production
ENV PATH=/app/node_modules/.bin:$PATH
ENV VITE_DISABLE_CHUNK_SPLITTING=true
ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8

WORKDIR /app
COPY ./web/package.json ./package.json
COPY ./web/bun.lockb ./bun.lockb

# Install dependencies and build
RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install --frozen-lockfile

COPY ./web .
COPY ./scripts ./scripts

# Install ARM64-specific dependencies only on ARM64 architecture
RUN if [ "$(uname -m)" = "aarch64" ]; then \
    bun add -d @rollup/rollup-linux-arm64-gnu@4.59.1; \
    fi

# Check and Build
RUN bun run check
RUN bun run build

# Production frontend dependencies only
FROM oven/bun:1.3.9@sha256:856da45d07aeb62eb38ea3e7f9e1794c0143a4ff63efb00e6c4491b627e2a521 AS web-runtime-deps
WORKDIR /app
COPY ./web/package.json ./package.json
COPY ./web/bun.lockb ./bun.lockb
RUN bun install --production --frozen-lockfile

# Final stage
FROM debian:bookworm-slim@sha256:67b30a61dc87758f0caf819646104f29ecbda97d920aaf5edc834128ac8493d3 AS runtime
LABEL org.opencontainers.image.source="https://github.com/erskingardner/keycast"
WORKDIR /app

# Install only the essential runtime dependencies
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates \
    netcat-openbsd \
    bash \
    curl \
    procps \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

ARG KEYCAST_UID=10001
ARG KEYCAST_GID=10001
RUN groupadd --system --gid "${KEYCAST_GID}" keycast \
    && useradd --system --uid "${KEYCAST_UID}" --gid keycast --home-dir /app --shell /usr/sbin/nologin keycast \
    && mkdir -p /app/database \
    && chown -R keycast:keycast /app

# Copy built artifacts (be more specific with the binary names)
COPY --chown=keycast:keycast --from=rust-builder /app/bin/keycast_api ./
COPY --chown=keycast:keycast --from=rust-builder /app/bin/keycast_signer ./
COPY --chown=keycast:keycast --from=rust-builder /app/bin/signer_daemon ./
COPY --chown=keycast:keycast ./database/migrations ./database/migrations
RUN mkdir -p /app/signer && chown keycast:keycast /app/signer
COPY --chown=keycast:keycast --from=web-builder /app/build ./web
COPY --chown=keycast:keycast --from=web-builder /app/package.json ./
COPY --chown=keycast:keycast --from=web-runtime-deps /app/node_modules ./node_modules
COPY --from=web-builder /usr/local/bin/bun /usr/local/bin/bun

# Set environment variables
ENV NODE_ENV=production \
    BUN_ENV=production \
    PATH=/usr/local/bin:$PATH

# Expose ports
EXPOSE 3000 5173

# Add a health check script
COPY scripts/healthcheck.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/healthcheck.sh

# Add an entrypoint script
COPY scripts/docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

USER keycast:keycast

HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD ["/usr/local/bin/healthcheck.sh"]

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["api"]
