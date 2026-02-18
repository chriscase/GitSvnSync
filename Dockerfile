FROM rust:1.83-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY crates/ crates/

# Build release binaries
RUN cargo build --release --bin gitsvnsync-daemon --bin gitsvnsync

# Verify binaries were produced and are executable
RUN test -x /app/target/release/gitsvnsync-daemon \
    && test -x /app/target/release/gitsvnsync \
    && /app/target/release/gitsvnsync --version

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    subversion \
    git \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /usr/sbin/nologin -m -d /var/lib/gitsvnsync gitsvnsync

# Copy binaries
COPY --from=builder /app/target/release/gitsvnsync-daemon /usr/local/bin/
COPY --from=builder /app/target/release/gitsvnsync /usr/local/bin/

# Copy default config (read-only for non-root)
COPY config.example.toml /etc/gitsvnsync/config.toml
RUN chmod 644 /etc/gitsvnsync/config.toml

# Create data directory
RUN mkdir -p /var/lib/gitsvnsync && chown gitsvnsync:gitsvnsync /var/lib/gitsvnsync

USER gitsvnsync
WORKDIR /var/lib/gitsvnsync

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s \
    CMD curl -f http://localhost:8080/api/status/health || exit 1

ENTRYPOINT ["gitsvnsync-daemon"]
CMD ["--config", "/etc/gitsvnsync/config.toml"]
