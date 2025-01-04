# Build stage for Rust API
FROM rustlang/rust:nightly AS rust-builder
WORKDIR /app
COPY ./api ./api
COPY ./signer ./signer
COPY ./core ./core
COPY ./Cargo.* .
COPY ./Cargo.lock .
RUN cargo build --release

# Build stage for Bun frontend
FROM oven/bun:1 AS web-builder
ENV NODE_OPTIONS="--max-old-space-size=1536"
WORKDIR /app
COPY ./web .
COPY ./scripts ./scripts

# Check/generate master.key before building
RUN if [ -f ../master.key ]; then \
    cp ../master.key .; \
    else \
    echo "No master.key found, will generate one"; \
    bun run scripts/generate_key.ts; \
    fi

# Install dependencies and build
RUN bun install
RUN bun --smol run build

# Install production dependencies only
RUN bun install --production

# Final stage
FROM debian:bookworm-slim
WORKDIR /app

# Install dependencies in a single layer and cleanup
RUN apt-get update && apt-get install -y \
    sqlite3 \
    curl \
    zip \
    unzip \
    ca-certificates \
    netcat-traditional \
    iproute2 \
    && rm -rf /var/lib/apt/lists/* \
    && curl -fsSL https://bun.sh/install | bash

# Create necessary directories
RUN mkdir -p /app/database

# Copy built artifacts
COPY --from=rust-builder /app/target/release/keycast_* ./
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
