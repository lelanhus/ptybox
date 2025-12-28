# Container Setup

This guide covers running ptybox in containerized environments (Docker, Podman, etc.).

## Why Containers?

On Linux, ptybox requires container isolation because the macOS Seatbelt sandbox is not available. The container provides:

- Filesystem isolation
- Network restrictions
- Process namespace separation
- Resource limits

## Quick Start

### Basic Docker Usage

```bash
# Build ptybox inside container
docker run --rm -v $PWD:/workspace -w /workspace rust:1.83 \
  cargo build --release -p ptybox-cli

# Run a command
docker run --rm -v $PWD:/workspace -w /workspace rust:1.83 \
  ./target/release/ptybox exec --json \
    --no-sandbox --ack-unsafe-sandbox \
    -- /bin/echo "hello"
```

### Policy for Containers

When running in containers, disable the Seatbelt sandbox and rely on container isolation:

```json
{
  "policy_version": 4,
  "sandbox": "none",
  "sandbox_unsafe_ack": true,
  "network": "disabled",
  "network_enforcement": {"unenforced_ack": true},
  "fs": {
    "allowed_read": ["/workspace", "/usr", "/lib", "/bin"],
    "allowed_write": ["/workspace/artifacts"],
    "working_dir": "/workspace"
  },
  "exec": {
    "allowed_executables": ["/bin/echo", "/usr/bin/env"],
    "allow_shell": false
  }
}
```

## Dockerfile Examples

### Minimal Runtime Image

```dockerfile
FROM debian:bookworm-slim

# Ensure /dev/pts is available for PTY allocation
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy pre-built binary
COPY ptybox /usr/local/bin/

WORKDIR /workspace
ENTRYPOINT ["ptybox"]
```

### Development Image

```dockerfile
FROM rust:1.83

# Install development dependencies
RUN apt-get update && apt-get install -y \
    jq \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

# Build ptybox from source
COPY . .
RUN cargo build --release -p ptybox-cli && \
    cp target/release/ptybox /usr/local/bin/

ENTRYPOINT ["ptybox"]
```

### Alpine-Based Minimal Image

```dockerfile
FROM rust:1.83-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /build
COPY . .
RUN cargo build --release -p ptybox-cli

FROM alpine:3.19

RUN apk add --no-cache libgcc

COPY --from=builder /build/target/release/ptybox /usr/local/bin/

WORKDIR /workspace
ENTRYPOINT ["ptybox"]
```

## Docker Run Options

### Essential Options

```bash
docker run --rm \
  -v $PWD:/workspace:rw \       # Mount your project
  -w /workspace \                # Set working directory
  --user $(id -u):$(id -g) \     # Match host user permissions
  ptybox-image exec ...
```

### PTY Support

If you encounter PTY allocation errors:

```bash
docker run --rm \
  --mount type=devpts,target=/dev/pts \
  -v $PWD:/workspace \
  ptybox-image exec ...
```

### Read-Only Root Filesystem

For extra security:

```bash
docker run --rm \
  --read-only \
  --tmpfs /tmp:rw,noexec,nosuid \
  -v $PWD:/workspace:rw \
  ptybox-image exec ...
```

## CI Integration

### GitHub Actions

```yaml
jobs:
  test:
    runs-on: ubuntu-latest
    container:
      image: rust:1.83
    steps:
      - uses: actions/checkout@v4

      - name: Build ptybox
        run: cargo build --release -p ptybox-cli

      - name: Run tests
        run: |
          ./target/release/ptybox run \
            --json \
            --no-sandbox --ack-unsafe-sandbox \
            --artifacts /tmp/artifacts \
            scenarios/test.yaml
```

### GitLab CI

```yaml
test:
  image: rust:1.83
  script:
    - cargo build --release -p ptybox-cli
    - ./target/release/ptybox run --json --no-sandbox --ack-unsafe-sandbox scenarios/
  artifacts:
    paths:
      - artifacts/
    when: always
```

## Podman Usage

Podman works similarly to Docker:

```bash
# Run with Podman
podman run --rm -v $PWD:/workspace:Z -w /workspace \
  rust:1.83 \
  ./target/release/ptybox exec --no-sandbox --ack-unsafe-sandbox -- /bin/echo "hello"
```

Note: Use `:Z` suffix on volumes for SELinux compatibility.

## Kubernetes

For Kubernetes environments:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: ptybox-test
spec:
  containers:
    - name: test
      image: your-registry/ptybox:latest
      command: ["ptybox", "run", "--json", "--no-sandbox", "--ack-unsafe-sandbox"]
      args: ["scenarios/test.yaml"]
      volumeMounts:
        - name: workspace
          mountPath: /workspace
  volumes:
    - name: workspace
      emptyDir: {}
```

## Security Considerations

### Container as Sandbox

When using `sandbox: none`, the container provides security boundaries:

1. **Filesystem**: Only mounted volumes are accessible
2. **Network**: Use `--network=none` for complete isolation
3. **Processes**: Container namespace limits visibility
4. **Resources**: Use `--memory` and `--cpus` limits

### Recommended Container Security

```bash
docker run --rm \
  --network=none \                    # No network access
  --read-only \                       # Read-only filesystem
  --security-opt=no-new-privileges \  # Prevent privilege escalation
  --cap-drop=ALL \                    # Drop all capabilities
  --memory=512m \                     # Limit memory
  --cpus=1 \                          # Limit CPU
  -v $PWD:/workspace:rw \
  ptybox-image exec ...
```

## Troubleshooting

### PTY Allocation Fails

```
Error: failed to open pty
```

Ensure devpts is available:

```bash
docker run --rm --mount type=devpts,target=/dev/pts ...
```

### Permission Denied on Volumes

Match the container user to your host user:

```bash
docker run --rm --user $(id -u):$(id -g) -v $PWD:/workspace ...
```

### Binary Not Found

For Alpine containers, ensure the binary is statically linked or glibc-compatible:

```bash
# Check binary dependencies
ldd /usr/local/bin/ptybox

# For musl systems, build with musl target
cargo build --release --target x86_64-unknown-linux-musl
```

See [Troubleshooting](troubleshooting.md) for more solutions.
