#!/usr/bin/env bash
# recached-6g-rust:local — the repo's Dockerfile builder (rust slim bookworm +
# build-essential) with the toolchain rust-toolchain.toml pins (1.98.0).
set -euo pipefail
NAME=recached-6g-rust-build
docker rm -f "$NAME" >/dev/null 2>&1 || true
docker run --name "$NAME" rust:1.98-slim-bookworm bash -euxc '
  apt-get update && apt-get install -y --no-install-recommends build-essential && rm -rf /var/lib/apt/lists/*
  rustup toolchain install 1.98.0 --profile minimal
  rustup default 1.98.0
  rustc --version'
docker commit "$NAME" recached-6g-rust:local
docker rm "$NAME"
