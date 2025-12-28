# Sandbox Security

ptybox enforces security through sandboxing and policy validation.

## Sandbox Backends

### Seatbelt (macOS)

The default on macOS. Uses Apple's sandbox-exec with a generated profile:

```json
{
  "sandbox": "seatbelt"
}
```

Enforces:
- Filesystem access restrictions
- Network access control
- Process execution limits

### None

Disables sandboxing (requires explicit acknowledgement):

```json
{
  "sandbox": "none",
  "sandbox_unsafe_ack": true
}
```

**Warning**: Without a sandbox, policy restrictions are not enforced at the OS level.

## Security Layers

### 1. Policy Validation

Before execution, ptybox validates:
- All paths are absolute
- No dangerous paths (/, $HOME, /System, etc.)
- Required acknowledgements present
- Executable is in allowlist

### 2. Sandbox Enforcement

The sandbox restricts the child process:
- File reads limited to `allowed_read`
- File writes limited to `allowed_write`
- Network blocked unless explicitly enabled
- Only allowed executables can run

### 3. Process Group Control

ptybox manages the entire process group:
- Child processes inherit restrictions
- Termination kills all descendants
- No orphan processes left behind

## Linux Containers

On Linux, use explicit `sandbox: none` with container isolation:

```bash
docker run --rm -v $(pwd):/work ptybox \
  exec --json --policy /work/policy.json -- /work/app
```

The container provides the security boundary.

## Best Practices

1. **Always use seatbelt on macOS** - It's the default for a reason
2. **Minimize allowed paths** - Only what's strictly necessary
3. **Never allow /tmp writes in production** - Use specific subdirectories
4. **Disable network by default** - Enable only when required
5. **Avoid shell execution** - Directly invoke executables

## Troubleshooting

### E_SANDBOX_UNAVAILABLE

The sandbox backend isn't available:
- On Linux, use `sandbox: none` with container isolation
- On macOS, ensure sandbox-exec is available

### E_POLICY_DENIED

Policy validation failed. Check:
- All paths are absolute
- No forbidden paths (/, $HOME)
- Required ack flags are set
- Executable is in allowlist
