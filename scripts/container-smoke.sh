#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACTS_DIR="${REPO_ROOT}/artifacts-container"

rm -rf "${ARTIFACTS_DIR}"
mkdir -p "${ARTIFACTS_DIR}"

IMAGE="rust:1.83"

# Paths inside the container (repo mounted at /work)
docker run --rm \
  -v "${REPO_ROOT}":/work \
  --workdir /work \
  "${IMAGE}" \
  /bin/bash -c '
    set -euo pipefail
    cargo build -p ptybox-cli
    ./target/debug/ptybox exec --json --policy /work/spec/examples/policy-container.json --artifacts /work/artifacts-container --overwrite -- /bin/echo hello
  '
