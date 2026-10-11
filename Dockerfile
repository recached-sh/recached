# ── Stage 1: Build ───────────────────────────────────────────────────────────
# Same compiler as rust-toolchain.toml; bump the two together.
FROM rust:1.98.0-slim-bookworm AS builder

# tikv-jemalloc-sys compiles jemalloc from C source, which needs a C toolchain
# (gcc + make). The slim image doesn't ship one.
RUN apt-get update && \
    apt-get install -y --no-install-recommends build-essential && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies separately so they aren't re-downloaded on every code change.
COPY Cargo.toml Cargo.lock ./
COPY core-engine/Cargo.toml    core-engine/Cargo.toml
COPY server-native/Cargo.toml  server-native/Cargo.toml
COPY sync-client/Cargo.toml    sync-client/Cargo.toml
COPY recached-embed/Cargo.toml recached-embed/Cargo.toml
COPY recached-mobile/Cargo.toml recached-mobile/Cargo.toml
COPY wasm-edge/Cargo.toml      wasm-edge/Cargo.toml

# Dummy source files so the dep-only build can resolve the workspace.
# sync-client/src, recached-embed/src, recached-mobile/src and wasm-edge/src are
# kept as stubs —
# Cargo parses every workspace member even when only building server-native,
# so each one's Cargo.toml and lib.rs must exist.
#
# Every new workspace member must be added to BOTH the manifest COPY block
# above and the stub list below. Omitting one fails the build here with
# "failed to load manifest for workspace member", which is how recached-embed
# and then recached-mobile (v0.4.0) broke this image after joining Cargo.toml.
# recached-mobile also declares a [[bin]] with an explicit path, so that file is
# stubbed too.
RUN mkdir -p core-engine/src server-native/src sync-client/src \
             recached-embed/src recached-mobile/src/bin wasm-edge/src && \
    echo "fn main() {}" > server-native/src/main.rs && \
    echo "" > core-engine/src/lib.rs && \
    echo "" > sync-client/src/lib.rs && \
    echo "" > recached-embed/src/lib.rs && \
    echo "" > recached-mobile/src/lib.rs && \
    echo "fn main() {}" > recached-mobile/src/bin/uniffi-bindgen.rs && \
    echo "" > wasm-edge/src/lib.rs && \
    cargo build --release --package recached && \
    rm -rf core-engine/src server-native/src

# Now copy real source and do the real build (only changed crates recompile).
COPY core-engine/src   core-engine/src
COPY server-native/src server-native/src

# COPY preserves source mtimes, which can be *older* than the dummy build
# above — cargo would then consider the crates unchanged and ship the dummy
# binary. Touch the roots so both crates always recompile.
RUN touch core-engine/src/lib.rs server-native/src/main.rs && \
    cargo build --release --package recached

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/recached-server /usr/local/bin/recached-server

EXPOSE 6379
EXPOSE 6380

CMD ["recached-server"]
