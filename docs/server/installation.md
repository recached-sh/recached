# Installation

## Docker

The fastest way to run Recached in any environment.

```bash
docker run -p 6379:6379 -p 6380:6380 ghcr.io/recached-sh/recached:latest
```

With environment variables for a production-like setup:

```bash
docker run \
  -p 6379:6379 \
  -p 6380:6380 \
  -e RECACHED_PASSWORD="your-secret" \
  -e RECACHED_MAX_KEYS="500000" \
  -e RECACHED_EVICTION="lru" \
  -e RECACHED_METRICS_PORT="9091" \
  -p 9091:9091 \
  ghcr.io/recached-sh/recached:latest
```

### Docker Compose

```yaml
services:
  recached:
    image: ghcr.io/recached-sh/recached:latest
    ports:
      - "6379:6379"
      - "6380:6380"
      - "9091:9091"
    environment:
      RECACHED_PASSWORD: "${RECACHED_PASSWORD}"
      RECACHED_MAX_KEYS: "1000000"
      RECACHED_EVICTION: "lru"
      RECACHED_METRICS_PORT: "9091"
      RUST_LOG: "info"
    restart: unless-stopped
```

---

## Homebrew (macOS)

```bash
# The tap is the main repo rather than a separate homebrew-recached
# repository, so the URL has to be given explicitly.
brew tap recached-sh/recached https://github.com/recached-sh/recached.git

# Homebrew 6.0 refuses to load formulae from an untrusted third-party tap.
# Skip this on older Homebrew, which has no `trust` subcommand.
brew trust recached-sh/recached

brew install recached
recached-server
```

To start Recached as a background service that restarts on login:

```bash
brew services start recached
```

To stop it:

```bash
brew services stop recached
```

---

## Cargo

Install from Git. The server package is marked `publish = false`:

```bash
cargo install --git https://github.com/recached-sh/recached.git recached
recached-server
```

This compiles from source and places `recached-server` in `~/.cargo/bin/`. Make sure `~/.cargo/bin` is in your `PATH`.

Set environment variables inline:

```bash
RECACHED_PASSWORD="secret" RECACHED_MAX_KEYS="100000" recached-server
```

---

## Building from source

Use the Rust toolchain pinned in `rust-toolchain.toml`. The workspace uses Rust edition 2024; building browser bindings also needs the `wasm32-unknown-unknown` target.

```bash
git clone https://github.com/recached-sh/recached.git
cd recached

# Build just the server binary
cargo build --release -p server-native

# The binary is at target/release/recached-server
./target/release/recached-server
```

To also build the WASM module:

```bash
# Install wasm-pack if you don't have it
cargo install wasm-pack

wasm-pack build wasm-edge --target web --out-dir ../wasm-edge/pkg
```

---

## Systemd service

For Linux servers where you want Recached to start on boot and restart on failure.

Create `/etc/systemd/system/recached.service`:

```ini
[Unit]
Description=Recached Cache Server
After=network.target
Wants=network.target

[Service]
Type=simple
User=recached
Group=recached
ExecStart=/usr/local/bin/recached-server
Restart=on-failure
RestartSec=5s

# Environment variables
Environment=RECACHED_PASSWORD=your-secret
Environment=RECACHED_MAX_KEYS=1000000
Environment=RECACHED_EVICTION=lru
Environment=RECACHED_METRICS_PORT=9091
Environment=RUST_LOG=info

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/recached

[Install]
WantedBy=multi-user.target
```

Create the user and enable the service:

```bash
# Create a dedicated user
sudo useradd --system --no-create-home --shell /bin/false recached

# Copy the binary
sudo cp target/release/recached-server /usr/local/bin/recached-server
sudo chmod 755 /usr/local/bin/recached-server

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable recached
sudo systemctl start recached

# Check status
sudo systemctl status recached
sudo journalctl -u recached -f
```

### Using an environment file

For sensitive values, use an environment file instead of putting secrets in the unit file:

`/etc/recached/recached.env`:

```bash
RECACHED_PASSWORD=your-very-secret-password
RECACHED_TLS_CERT=/etc/recached/tls/cert.pem
RECACHED_TLS_KEY=/etc/recached/tls/key.pem
```

Update the service file:

```ini
[Service]
EnvironmentFile=/etc/recached/recached.env
ExecStart=/usr/local/bin/recached-server
```

```bash
sudo chmod 600 /etc/recached/recached.env
sudo systemctl daemon-reload
sudo systemctl restart recached
```

---

## Verifying the installation

Once running, test with any Redis CLI:

```bash
# Basic connectivity
redis-cli -h 127.0.0.1 -p 6379 ping
# PONG

# With auth
redis-cli -h 127.0.0.1 -p 6379 -a your-secret ping
# PONG

# Basic operations
redis-cli -h 127.0.0.1 -p 6379 set hello world
redis-cli -h 127.0.0.1 -p 6379 get hello
# "world"
```

Check the Prometheus metrics endpoint (if `RECACHED_METRICS_PORT` is set):

```bash
curl http://127.0.0.1:9091/metrics
```
