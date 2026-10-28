#!/usr/bin/env bash
# Install the Routstr daemon used by the Routstr / Routstr Confidential providers.
#
#   ./scripts/install-routstrd.sh              # latest stable release -> ~/.local/bin
#   ./scripts/install-routstrd.sh --bundle     # also copy into resources/bin/routstrd
#   ./scripts/install-routstrd.sh --confidential
#       Build the confidential-upstream fork from source (requires bun).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUNDLE=0
CONFIDENTIAL=0
for arg in "$@"; do
  case "$arg" in
    --bundle) BUNDLE=1 ;;
    --confidential) CONFIDENTIAL=1 ;;
    -h|--help)
      sed -n '2,12p' "$0"
      exit 0
      ;;
    *) echo "unknown option: $arg" >&2; exit 1 ;;
  esac
done

WITH_BUN=0
command -v bun >/dev/null 2>&1 && WITH_BUN=1

if [[ "$CONFIDENTIAL" == "1" ]]; then
  if [[ "$WITH_BUN" == "0" ]]; then
    echo "error: building the confidential-upstream fork requires bun (https://bun.sh)" >&2
    exit 1
  fi
  WORK="$(mktemp -d)"
  trap 'rm -rf "$WORK"' EXIT
  echo "==> Cloning jooray/routstrd (confidential-upstream)"
  git clone --depth 1 --branch confidential-upstream https://github.com/jooray/routstrd.git "$WORK/routstrd"
  echo "==> Cloning and building @routstr/sdk (confidential-upstream)"
  git clone --depth 1 --branch confidential-upstream https://github.com/jooray/routstr-sdk.git "$WORK/routstr-sdk"
  (cd "$WORK/routstr-sdk" && bun install && bun run build)
  echo "==> Installing dependencies and building a standalone binary"
  (cd "$WORK/routstrd" && bun install --frozen-lockfile 2>/dev/null || bun install)
  # Substitute the unreleased confidential SDK build for the published package.
  mkdir -p "$WORK/routstrd/node_modules/@routstr"
  ln -sfn "$WORK/routstr-sdk" "$WORK/routstrd/node_modules/@routstr/sdk"
  (cd "$WORK/routstrd" && bun add reflect-metadata)
  # The CLI entry must import reflect-metadata before anything else.
  if ! grep -q 'reflect-metadata' "$WORK/routstrd/src/index.ts"; then
    sed -i '1a import "reflect-metadata";' "$WORK/routstrd/src/index.ts"
  fi
  (cd "$WORK/routstrd" && bun run build:binary)
  BIN="$(find "$WORK/routstrd/dist" -maxdepth 2 -type f -name 'routstrd' | head -n1)"
  if [[ -z "${BIN:-}" || ! -x "$BIN" ]]; then
    echo "error: build did not produce dist/routstrd" >&2
    exit 1
  fi
  install -Dm755 "$BIN" "$HOME/.local/bin/routstrd"
else
  ARCH="$(uname -m)"
  case "$ARCH" in
    x86_64|amd64) ASSET_ARCH=x64 ;;
    aarch64|arm64) ASSET_ARCH=arm64 ;;
    *) echo "unsupported architecture: $ARCH" >&2; exit 1 ;;
  esac
  OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
  echo "==> Downloading latest routstrd release for ${OS}-${ASSET_ARCH}"
  curl -fsSL https://github.com/Routstr/routstrd/releases/latest/download/install.sh | sh
fi

if [[ "$BUNDLE" == "1" ]]; then
  mkdir -p "$ROOT/resources/bin"
  cp "$(command -v routstrd || echo "$HOME/.local/bin/routstrd")" "$ROOT/resources/bin/routstrd"
  chmod 755 "$ROOT/resources/bin/routstrd"
  echo "==> Bundled routstrd into resources/bin/routstrd"
fi

echo
echo "routstrd installed. In QuixiDesk open Providers -> Routstr -> Start Daemon."
echo "For confidential mode install cu-prover + cu-2pc:"
echo "  git clone https://github.com/jooray/routstr-confidential"
echo "  cd routstr-confidential/crypto/zkbench && cargo build --release --bin cu-prover --bin cu-2pc"
echo "  cp target/release/cu-prover target/release/cu-2pc ~/.local/bin/"
