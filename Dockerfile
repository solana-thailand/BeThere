# ============================================
# Stage 1: Build the Rust binary
# ============================================
FROM rust:1.85-bookworm AS builder

WORKDIR /app

# Cache dependencies by copying manifests first
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies cache layer
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies only (this layer is cached unless Cargo.toml/lock changes)
RUN cargo build --release && rm -rf src

# Copy the actual source code
COPY src/ src/

# Touch main.rs to ensure rebuild (not cached dummy)
RUN touch src/main.rs

# Build the actual binary
RUN cargo build --release

# ============================================
# Stage 2: Minimal runtime image
# ============================================
FROM debian:bookworm-slim AS runtime

# Install ca-certificates for HTTPS (Google API calls) and timezone data
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        tzdata && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd --gid 1000 appuser && \
    useradd --uid 1000 --gid appuser --shell /bin/bash --create-home appuser

WORKDIR /app

# Copy the compiled binary from builder
COPY --from=builder /app/target/release/event-checkin .

# Copy frontend static assets
COPY frontend/ ./frontend/

# Set ownership to non-root user
RUN chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# Expose the default port
EXPOSE 3000

# Environment defaults (overridden at runtime via docker run -e or docker-compose)
ENV HOST=0.0.0.0
ENV PORT=3000
ENV RUST_LOG=event_checkin=info,tower_http=warn

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/api/health || exit 1

# Run the binary
ENTRYPOINT ["./event-checkin"]
