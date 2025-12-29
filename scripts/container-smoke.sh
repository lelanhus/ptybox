#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACTS_DIR="${REPO_ROOT}/artifacts-container"
POLICY_PATH="${REPO_ROOT}/spec/examples/policy-container.json"

rm -rf "${ARTIFACTS_DIR}"
mkdir -p "${ARTIFACTS_DIR}"

IMAGE="rust:1.83"

docker run --rm \
  -v "${REPO_ROOT}":/work \
  -w /work \
  "${IMAGE}" \
  bash -lc "
    set -euo pipefail
    cargo build -p ptybox-cli
    ./target/debug/ptybox exec --json --policy ${POLICY_PATH} --artifacts ${ARTIFACTS_DIR} -- /bin/echo hello
  "
