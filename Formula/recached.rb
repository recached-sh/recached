class Recached < Formula
  desc "Local-first cache server with a Redis-compatible RESP subset and WebSocket sync"
  homepage "https://github.com/recached-sh/recached"
  version "0.5.0"
  license "Apache-2.0"

  # The checksums below are placeholders until this version's release artifacts
  # exist. Fill them with `scripts/update-formula-checksums.sh v<version>`
  # (matching the `version` above), which downloads the published binaries and
  # rewrites this file.
  #
  # `scripts/bump-version.sh` resets them to placeholders on every bump, and
  # that is deliberate: this formula sat at 0.1.8 with *valid* 0.1.8 URLs and
  # checksums while the project shipped 0.2.x, so `brew install recached`
  # silently succeeded and handed people the old binary — including the one
  # whose replication port served the keyspace without authentication. A
  # placeholder makes brew fail loudly, which is the far better failure.
  on_macos do
    on_intel do
      url "https://github.com/recached-sh/recached/releases/download/v0.5.0/recached-macos-amd64"
      sha256 "80273af270b3c24f4cb3a090dbcd430b45168c00208f9b09091440c1b5c5a727"
    end
    on_arm do
      url "https://github.com/recached-sh/recached/releases/download/v0.5.0/recached-macos-arm64"
      sha256 "7f78bb5b2b4047d62f59c00d8160376b3a1dfc04ff4456b4b8ea8fb94f5cf18d"
    end
  end

  def install
    # Rename the downloaded binary to 'recached-server' and install it into the Homebrew bin
    binary = Hardware::CPU.arm? ? "recached-macos-arm64" : "recached-macos-amd64"
    bin.install binary => "recached-server"
  end

  # This allows users to run `brew services start recached` to run it automatically in the background
  service do
    run opt_bin/"recached-server"
    keep_alive true
    error_log_path var/"log/recached.error.log"
    log_path var/"log/recached.log"
  end

  test do
    # Simple test to verify the binary installed correctly
    system "#{bin}/recached-server", "--version"
  end
end
