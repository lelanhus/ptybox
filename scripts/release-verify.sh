#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/release-verify.sh verify-artifacts <artifacts-dir>
  scripts/release-verify.sh generate-checksums <artifacts-dir> <checksums-file>
  scripts/release-verify.sh verify-checksums <artifacts-dir> <checksums-file>

Environment:
  RELEASE_TARGETS  Space-separated target list. Defaults to:
                   x86_64-apple-darwin aarch64-apple-darwin
                   x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
EOF
}

log() {
  printf '%s\n' "$*"
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

hash_generate() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$@"
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$@"
  else
    die "neither sha256sum nor shasum is available"
  fi
}

hash_check() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$1"
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$1"
  else
    die "neither sha256sum nor shasum is available"
  fi
}

verify_artifacts() {
  local artifacts_dir="$1"
  [ -d "$artifacts_dir" ] || die "artifacts directory does not exist: $artifacts_dir"

  local targets="${RELEASE_TARGETS:-x86_64-apple-darwin aarch64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu}"
  local missing=()
  local target
  for target in $targets; do
    local expected_name="ptybox-${target}.tar.gz"
    local found
    found="$(find "$artifacts_dir" -type f -name "$expected_name" | head -n 1)"
    if [ -z "$found" ]; then
      missing+=("$expected_name")
      continue
    fi

    if [ ! -s "$found" ]; then
      die "artifact is empty: $found"
    fi

    if ! tar -tzf "$found" | grep -Eq '^(\./)?ptybox$'; then
      die "artifact does not contain ptybox binary entry: $found"
    fi

    log "verified artifact: $found"
  done

  if [ "${#missing[@]}" -gt 0 ]; then
    printf 'missing expected artifacts:\n' >&2
    printf '  - %s\n' "${missing[@]}" >&2
    exit 1
  fi
}

generate_checksums() {
  local artifacts_dir="$1"
  local checksums_file="$2"
  [ -d "$artifacts_dir" ] || die "artifacts directory does not exist: $artifacts_dir"

  local tarballs=()
  while IFS= read -r file; do
    tarballs+=("$file")
  done < <(cd "$artifacts_dir" && find . -type f -name 'ptybox-*.tar.gz' | sort)

  [ "${#tarballs[@]}" -gt 0 ] || die "no release tarballs found in $artifacts_dir"

  mkdir -p "$(dirname "$checksums_file")"
  (
    cd "$artifacts_dir"
    hash_generate "${tarballs[@]}"
  ) >"$checksums_file"

  [ -s "$checksums_file" ] || die "checksums file is missing or empty: $checksums_file"
  log "generated checksums at: $checksums_file"
  cat "$checksums_file"
}

verify_checksums() {
  local artifacts_dir="$1"
  local checksums_file="$2"
  [ -d "$artifacts_dir" ] || die "artifacts directory does not exist: $artifacts_dir"
  [ -s "$checksums_file" ] || die "checksums file is missing or empty: $checksums_file"

  local artifacts_abs checksums_abs
  artifacts_abs="$(cd "$artifacts_dir" && pwd)"
  checksums_abs="$(cd "$(dirname "$checksums_file")" && pwd)/$(basename "$checksums_file")"

  local checksums_name
  checksums_name="$(basename "$checksums_file")"
  if [ "$checksums_abs" != "$artifacts_abs/$checksums_name" ]; then
    cp "$checksums_file" "$artifacts_dir/$checksums_name"
  fi
  (
    cd "$artifacts_dir"
    hash_check "$checksums_name"
  )
  log "verified checksums from: $checksums_file"
}

main() {
  if [ "$#" -lt 2 ]; then
    usage
    exit 1
  fi

  local command="$1"
  shift
  case "$command" in
  verify-artifacts)
    [ "$#" -eq 1 ] || die "verify-artifacts expects <artifacts-dir>"
    verify_artifacts "$1"
    ;;
  generate-checksums)
    [ "$#" -eq 2 ] || die "generate-checksums expects <artifacts-dir> <checksums-file>"
    generate_checksums "$1" "$2"
    ;;
  verify-checksums)
    [ "$#" -eq 2 ] || die "verify-checksums expects <artifacts-dir> <checksums-file>"
    verify_checksums "$1" "$2"
    ;;
  *)
    usage
    exit 1
    ;;
  esac
}

main "$@"
