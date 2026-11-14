#!/usr/bin/env bash
# Builds recached-6g-bench:local — Debian bookworm with Redis, Valkey and
# memtier_benchmark compiled from upstream source, all with one compiler and
# libc. Redis and Valkey use their default Linux allocator (bundled jemalloc).
#
# Uses `docker run` + `docker commit` rather than `docker build` so that only
# the locally cached debian:bookworm-slim is needed (BuildKit resolves the
# base image against the registry, which was unreachable from this host).
set -euo pipefail
REDIS_VERSION=${REDIS_VERSION:-7.2.5}
VALKEY_VERSION=${VALKEY_VERSION:-9.1.0}
MEMTIER_VERSION=${MEMTIER_VERSION:-2.1.4}
NAME=recached-6g-bench-build
docker rm -f "$NAME" >/dev/null 2>&1 || true
docker run --name "$NAME" debian:bookworm-slim bash -euxc "
  apt-get update
  apt-get install -y --no-install-recommends build-essential pkg-config curl ca-certificates \
    libevent-dev libssl-dev autoconf automake libtool libpcre3-dev zlib1g-dev \
    procps util-linux python3 jq
  mkdir -p /build && cd /build
  curl -fsSL https://download.redis.io/releases/redis-$REDIS_VERSION.tar.gz | tar xz
  make -C redis-$REDIS_VERSION -j\$(nproc) BUILD_TLS=no
  install redis-$REDIS_VERSION/src/redis-server redis-$REDIS_VERSION/src/redis-cli redis-$REDIS_VERSION/src/redis-benchmark /usr/local/bin/
  curl -fsSL https://github.com/valkey-io/valkey/archive/refs/tags/$VALKEY_VERSION.tar.gz | tar xz
  make -C valkey-$VALKEY_VERSION -j\$(nproc) BUILD_TLS=no
  install valkey-$VALKEY_VERSION/src/valkey-server /usr/local/bin/
  curl -fsSL https://github.com/RedisLabs/memtier_benchmark/archive/refs/tags/$MEMTIER_VERSION.tar.gz | tar xz
  cd memtier_benchmark-$MEMTIER_VERSION && autoreconf -ivf && ./configure && make -j\$(nproc) && make install
  cd / && rm -rf /build /var/lib/apt/lists/*
  redis-server --version; valkey-server --version; memtier_benchmark --version
"
docker commit "$NAME" recached-6g-bench:local
docker rm "$NAME"
