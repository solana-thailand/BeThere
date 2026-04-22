# ============================================
# Stage 1: Build Leptos WASM frontend
# ============================================
FROM rust:1.85-bookworm AS frontend-builder

RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk

WORKDIR /app

# Cache frontend Rust dependencies
COPY frontend-leptos/Cargo.toml frontend-leptos/Cargo.lock ./
RUN mkdir src && echo "" > src/lib.rs
RUN CARGO_BUILD_JOBS=1 cargo build --target wasm32-unknown-unknown --release 2>/dev/null || true

# Copy frontend source and build
COPY frontend-leptos/ ./
RUN CARGO_BUILD_JOBS=1 trunk build --release

# Clean trunk artifacts (nonces, live-reload script, template vars)
RUN sed -i 's/ nonce="[^"]*"//g' dist/index.html && \
    sed -i '/{{__TRUNK/d' dist/index.html

# ============================================
# Stage 2: Build Rust backend
# ============================================
FROM rust:1.85-bookworm AS backend-builder

WORKDIR /app

# Cache backend dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Copy actual source and build
COPY src/ src/
RUN touch src/main.rs && cargo build --release

# ============================================
# Stage 3: Minimal runtime
# ============================================
FROM debian:bookworm-slim AS runtime

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates tzdata curl && \
    rm -rf /var/lib/apt/lists/*

RUN groupadd --gid 1000 appuser && \
    useradd --uid 1000 --gid appuser --shell /bin/bash --create-home appuser

WORKDIR /app

COPY --from=backend-builder /app/target/release/event-checkin .
COPY --from=frontend-builder /app/dist ./frontend-leptos/dist/

RUN chown -R appuser:appuser /app
USER appuser

EXPOSE 3000

ENV HOST=0.0.0.0
ENV PORT=3000
ENV RUST_LOG=event_checkin=info,tower_http=warn

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/api/health || exit 1

ENTRYPOINT ["./event-checkin"]
