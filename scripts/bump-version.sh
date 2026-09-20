#!/usr/bin/env bash
# Usage: ./scripts/bump-version.sh <new-version>
# Updates Cargo workspace metadata plus every published npm manifest and lockfile.
set -euo pipefail

NEW_VERSION="${1:?Usage: $0 <new-version>  e.g. $0 0.2.0}"

# Basic semver guard (0.1.0 / 1.0.0-rc.1 / 2.3.4-beta.5)
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?(\+[a-zA-Z0-9.]+)?$ ]]; then
    echo "error: '$NEW_VERSION' is not a valid semver string" >&2
    exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CARGO_TOML="$ROOT/Cargo.toml"

CURRENT=$(grep '^version' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/')

# Replace the version line inside [workspace.package] — only the first occurrence.
python3 - "$CARGO_TOML" "$NEW_VERSION" <<'EOF'
import re, sys
path, version = sys.argv[1], sys.argv[2]
content = open(path).read()
updated, n = re.subn(r'^version\s*=\s*"[^"]+"', f'version = "{version}"', content, count=1, flags=re.MULTILINE)
if n == 0:
    print("error: could not find 'version = ...' in " + path, file=sys.stderr)
    sys.exit(1)
open(path, 'w').write(updated)
EOF

echo "Bumped $CURRENT → $NEW_VERSION in $CARGO_TOML"

# Update package.json files.
update_package_json() {
    local pkg="$1"
    python3 - "$pkg" "$NEW_VERSION" <<'PYEOF'
import json, os, sys
path, version = sys.argv[1], sys.argv[2]
with open(path) as f:
    data = json.load(f)
data['version'] = version
with open(path, 'w') as f:
    json.dump(data, f, indent=2)
    f.write('\n')

lock_path = os.path.join(os.path.dirname(path), 'package-lock.json')
if os.path.exists(lock_path):
    with open(lock_path) as f:
        lock = json.load(f)
    lock['version'] = version
    if '' in lock.get('packages', {}):
        lock['packages']['']['version'] = version
    with open(lock_path, 'w') as f:
        json.dump(lock, f, indent=2)
        f.write('\n')
PYEOF
    echo "Bumped $NEW_VERSION in $pkg and its lockfile"
}

update_package_json "$ROOT/wasm-edge/package.json"
update_package_json "$ROOT/sdks/recached-react/package.json"
update_package_json "$ROOT/sdks/recached-vue/package.json"

# Update the Homebrew formula: version, download URLs, and — deliberately —
# reset the checksums to placeholders.
#
# This file used to be bumped by hand, so it wasn't: it sat at 0.1.8 with valid
# 0.1.8 URLs and checksums while the project shipped 0.2.x, and `brew install`
# quietly served the old binary. Resetting the sums means the formula cannot
# install anything until `scripts/update-formula-checksums.sh v$NEW_VERSION` has
# been run against the real release artifacts.
python3 - "$ROOT/Formula/recached.rb" "$NEW_VERSION" <<'EOF'
import re, sys
path, version = sys.argv[1], sys.argv[2]
content = open(path).read()

content, n = re.subn(r'^(  version )"[^"]+"', rf'\1"{version}"', content, count=1, flags=re.MULTILINE)
if n == 0:
    sys.exit("error: could not find the version line in " + path)

content, n = re.subn(r'/releases/download/v[^/]+/', f'/releases/download/v{version}/', content)
if n == 0:
    sys.exit("error: could not find any release download URL in " + path)
urls = n

# Any real checksum becomes a placeholder again; existing placeholders are left
# alone. A sha256 line is 64 hex chars.
content, x86 = re.subn(r'(on_intel do.*?sha256 )"[0-9a-f]{64}"',
                       r'\1"REPLACE_WITH_AMD64_SHA256"', content, flags=re.DOTALL)
content, arm = re.subn(r'(on_arm do.*?sha256 )"[0-9a-f]{64}"',
                       r'\1"REPLACE_WITH_ARM64_SHA256"', content, flags=re.DOTALL)

open(path, 'w').write(content)
print(f"Bumped {version} in {path} ({urls} URL(s); reset {x86 + arm} checksum(s) to placeholders)")
EOF

echo
echo "NOTE: Formula/recached.rb now carries placeholder checksums."
echo "      After the v$NEW_VERSION release artifacts are published, run:"
echo "        scripts/update-formula-checksums.sh v$NEW_VERSION"

# Verify the workspace resolves cleanly.
echo "Verifying workspace..."
cargo check --workspace --exclude wasm-edge --quiet
echo "OK — all crates resolved at $NEW_VERSION"
