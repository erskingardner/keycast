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
WORKDIR /app
COPY ./web .
RUN bun install
RUN bun run build

# Final stage
FROM debian:bookworm-slim
WORKDIR /app

# Install dependencies in a single layer and cleanup
RUN apt-get update && apt-get install -y \
    supervisor \
    sqlite3 \
    curl \
    zip \
    unzip \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && curl -fsSL https://bun.sh/install | bash

# Create necessary directories
RUN mkdir -p /app/database

# Copy built artifacts
COPY --from=rust-builder /app/target/release/keycast_* ./
COPY --from=web-builder /app/.svelte-kit/output ./web
COPY supervisord.conf /etc/supervisor/conf.d/supervisord.conf

# Generate master.key if it doesn't exist
RUN if [ ! -f "./master.key" ]; then \
    echo "Generating new master key..." && \
    /root/.bun/bin/bun run key:generate; \
    fi

# Set environment variables
ENV NODE_ENV=production \
    BUN_ENV=production

# Expose ports
EXPOSE 3000 5173

# Add healthcheck
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Use supervisor to manage processes
CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/conf.d/supervisord.conf"]
