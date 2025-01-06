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
FROM oven/bun:1-slim AS web-builder
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
FROM alpine:3.19 AS runtime
WORKDIR /app

# Install only the essential runtime dependencies
RUN apk add --no-cache \
    sqlite \
    ca-certificates \
    netcat-openbsd \
    bash \
    curl

RUN curl -fsSL https://bun.sh/install | bash

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
