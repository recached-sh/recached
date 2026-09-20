class Recached < Formula
  desc "Blazing fast, multi-core drop-in replacement for Redis"
  homepage "https://github.com/recached-sh/recached"
  version "0.3.5"
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
      url "https://github.com/recached-sh/recached/releases/download/v0.3.5/recached-macos-amd64"
      sha256 "REPLACE_WITH_AMD64_SHA256"
    end
    on_arm do
      url "https://github.com/recached-sh/recached/releases/download/v0.3.5/recached-macos-arm64"
      sha256 "REPLACE_WITH_ARM64_SHA256"
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
