# Troubleshooting

Common issues and their solutions when using ptybox.

## Quick Diagnosis

When something goes wrong, gather this information:

```bash
# Check the exit code
echo $?

# Run with JSON output for structured error info
ptybox exec --json ... | jq .error

# Check artifact directory for debug info
ls -la /path/to/artifacts/
cat /path/to/artifacts/run.json | jq .
```

## Common Errors

### E_SANDBOX_UNAVAILABLE (exit code 3)

**Symptom:** Error message about sandbox not being available.

**Causes:**
- Running on Linux without proper container isolation
- Seatbelt not accessible on macOS (rare)

**Solutions:**

On Linux, use container isolation instead of Seatbelt:

```json
{
  "sandbox": "none",
  "sandbox_unsafe_ack": true
}
```

Then run inside a container (Docker, Podman, etc.) that provides isolation:

```bash
docker run --rm -v $PWD:/workspace ptybox exec --policy policy.json -- ./app
```

### E_POLICY_DENIED (exit code 2)

**Symptom:** Error message about policy validation failure.

**Diagnosis:** Run with `--explain-policy` to see what's being validated:

```bash
ptybox exec --explain-policy --policy policy.json -- ./app
```

**Common causes and fixes:**

| Cause | Error Message | Fix |
|-------|---------------|-----|
| Executable not allowed | "not in allowed_executables" | Add to `exec.allowed_executables` |
| Relative path | "path must be absolute" | Use absolute paths like `/usr/bin/app` |
| Dangerous path | "forbidden path" | Avoid `/`, `$HOME`, `/System` |
| Missing write ack | "write_ack required" | Set `fs.write_ack: true` |
| Missing sandbox ack | "sandbox_unsafe_ack required" | Set `sandbox_unsafe_ack: true` |

### E_TIMEOUT (exit code 4)

**Symptom:** Operation times out before completion.

**Diagnosis:** Check if the wait condition was ever satisfied:

```bash
cat artifacts/events.jsonl | jq 'select(.transcript_delta != null)'
```

**Fixes:**

1. Increase step timeout:
```json
{
  "steps": [
    {"action": {...}, "timeout_ms": 30000}
  ]
}
```

2. Increase total budget:
```json
{
  "budgets": {"total_runtime_ms": 120000}
}
```

3. Fix your wait condition (text may appear differently than expected).

### PTY Allocation Errors

**Symptom:** "failed to open pty" or similar message.

**Causes:**
- Running in a minimal container without `/dev/pts`
- Insufficient permissions

**Solutions:**

In Docker, ensure `/dev/pts` is available:

```bash
docker run --rm -it \
  -v /dev/pts:/dev/pts \
  --privileged \
  ...
```

Or use a Dockerfile that sets up pseudo-terminal support:

```dockerfile
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y procps
```

### Empty Screen Snapshots

**Symptom:** Screen snapshot shows no content.

**Causes:**
- Application outputs to stderr instead of stdout
- Application exits before output is captured
- Terminal not properly sized

**Fixes:**

1. Add a wait after launching:
```json
{"steps": [
  {"action": {"type": "wait", "payload": {"condition": {"type": "delay", "payload": {"ms": 100}}}}}
]}
```

2. Check if application requires specific terminal size:
```json
{"terminal_size": {"rows": 24, "cols": 80}}
```

3. Verify application output by checking transcript:
```bash
cat artifacts/events.jsonl | jq -r '.transcript_delta // empty'
```

### Assertion Failures

**Symptom:** E_ASSERTION_FAILED with unexpected content.

**Diagnosis:**

1. View the actual screen content:
```bash
cat artifacts/snapshots/step_*/snapshot.json | jq -r '.lines[]'
```

2. Compare with expectation:
```bash
ptybox trace --open artifacts/
```

**Common issues:**
- Leading/trailing whitespace in expected text
- ANSI color codes in output (use `strip_ansi` normalization)
- Case sensitivity (assertions are case-sensitive by default)

### Container-Specific Issues

#### /dev/pts Not Mounted

**Symptom:** "failed to allocate pty" in container.

**Fix:** Mount the devpts filesystem:

```bash
docker run --rm \
  --mount type=devpts,target=/dev/pts \
  ...
```

#### Permission Denied in Container

**Symptom:** Cannot read/write files from mounted volumes.

**Fix:** Ensure correct user permissions:

```bash
docker run --rm \
  --user $(id -u):$(id -g) \
  -v $PWD:/workspace \
  ...
```

## Debug Techniques

### Verbose Output

Run with verbose mode for extra logging:

```bash
ptybox exec --verbose --json ... 2>debug.log
```

### Trace Viewer

Generate an HTML trace for visual debugging:

```bash
ptybox trace artifacts/ --output trace.html
open trace.html  # or xdg-open on Linux
```

### Manual Inspection

Check artifacts for debugging:

```bash
# Overall run result
cat artifacts/run.json | jq .

# Step-by-step events
cat artifacts/events.jsonl | head -20

# Screen at each step
ls artifacts/snapshots/
cat artifacts/snapshots/step_001/snapshot.json | jq -r '.lines[]'

# Checksums for replay verification
cat artifacts/checksums.json
```

### Interactive Driver Mode

Test actions interactively:

```bash
ptybox driver --stdio --json -- ./app
```

Then send actions manually:

```json
{"protocol_version": 1, "action": {"type": "text", "payload": {"text": "hello"}}}
```

## Getting Help

1. Check the [error codes reference](../reference/error-codes.md) for detailed explanations
2. Review your policy with `--explain-policy`
3. Generate a trace file for visual debugging
4. Check GitHub issues for similar problems
