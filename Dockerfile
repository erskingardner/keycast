FROM rustlang/rust:nightly as rust-builder
WORKDIR /app
COPY ./api ./api
COPY ./signer ./signer
COPY ./core ./core
COPY ./permissions ./permissions
COPY ./Cargo.* .
COPY ./Cargo.lock .
RUN cargo build --release

FROM oven/bun:1 as web-builder
WORKDIR /app
COPY ./web .
RUN bun install
RUN bun run build

# Final stage
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y supervisor sqlite3 curl zip unzip
RUN curl -fsSL https://bun.sh/install | bash
COPY --from=rust-builder /app/target/release/keycast_* ./
COPY --from=web-builder /app/.svelte-kit/output ./web
COPY supervisord.conf /etc/supervisor/conf.d/
CMD ["/usr/bin/supervisord"]
