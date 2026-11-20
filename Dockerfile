# ── Etapa 1: Compilación ──────────────────────────────────────────────────────
FROM docker.io/library/rust:alpine3.23 AS builder

RUN apk add --no-cache --update \
            build-base \
            autoconf \
            gdb \
            musl-dev \
            pkgconfig \
            strace \
            openssl \
            openssl-dev \
            openssl-libs-static \
            cmake \
            clang \
            perl

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

RUN cargo build --release && \
    strip target/release/semaphore-mcp

# ── Etapa 2: Runtime ──────────────────────────────────────────────────────────
FROM docker.io/library/alpine:3.23

RUN apk add --update --no-cache \
    ca-certificates \
    curl \
    && \
    adduser -S -u 1000 -D mcp

COPY --from=builder /app/target/release/semaphore-mcp /usr/local/bin/

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD pgrep -x semaphore-mcp || exit 1

USER mcp
EXPOSE 8090

CMD ["semaphore-mcp"]
