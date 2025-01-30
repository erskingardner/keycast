# Build stage for Rust API
FROM rustlang/rust:nightly-slim AS rust-builder
WORKDIR /app
COPY ./api ./api
COPY ./signer ./signer
COPY ./core ./core
COPY ./Cargo.* .
COPY ./Cargo.lock .
RUN cargo build --release

# Build stage for Bun frontend
FROM oven/bun:1 AS web-builder

# Install build essentials for native modules
RUN apt-get update && apt-get install -y \
    python3 \
    python-is-python3 \
    make \
    g++ \
    && rm -rf /var/lib/apt/lists/*

ARG VITE_DOMAIN
ENV VITE_DOMAIN=$VITE_DOMAIN
ARG VITE_ALLOWED_PUBKEYS
ENV VITE_ALLOWED_PUBKEYS=$VITE_ALLOWED_PUBKEYS

ENV NODE_OPTIONS="--max-old-space-size=2048"
ENV CI=true
ENV NODE_ENV=production
ENV VITE_BUILD_MODE=production
ENV PATH=/app/node_modules/.bin:$PATH
ENV VITE_DISABLE_CHUNK_SPLITTING=true
ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8

WORKDIR /app
COPY ./web .
COPY ./scripts ./scripts
COPY master.key .

# Install dependencies and build
RUN bun install

# Install ARM64-specific dependencies only on ARM64 architecture
RUN if [ "$(uname -m)" = "aarch64" ]; then \
    bun add -d @rollup/rollup-linux-arm64-gnu; \
    fi

# Check and Build
RUN bun run check
RUN bun run build

# Final stage
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Install only the essential runtime dependencies
RUN apt-get update && apt-get install -y \
    sqlite3 \
    ca-certificates \
    netcat-openbsd \
    bash \
    curl \
    unzip \
    iproute2 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install Bun for use in the entrypoint script
RUN curl -fsSL https://bun.sh/install | bash

# Create necessary directories
RUN mkdir -p /app/database

# Copy built artifacts (be more specific with the binary names)
COPY --from=rust-builder /app/target/release/keycast_api ./
COPY --from=rust-builder /app/target/release/keycast_signer ./
COPY --from=rust-builder /app/target/release/signer_daemon ./
COPY --from=web-builder /app/master.key ./
COPY --from=web-builder /app/build ./web
COPY --from=web-builder /app/package.json ./
COPY --from=web-builder /app/node_modules ./node_modules

# Set environment variables
ENV NODE_ENV=production \
    BUN_ENV=production \
    PATH=/root/.bun/bin:$PATH

# Expose ports
EXPOSE 3000 5173

# Add a health check script
COPY scripts/healthcheck.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/healthcheck.sh

# Add an entrypoint script
COPY scripts/docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD ["/usr/local/bin/healthcheck.sh"]

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["api"]
