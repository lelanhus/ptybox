# Policies

Policies define security constraints for command execution. ptybox uses a **deny-by-default** model.

## Minimal Policy

```json
{
  "policy_version": 4,
  "sandbox": "none",
  "sandbox_unsafe_ack": true,
  "network": "disabled",
  "network_unsafe_ack": true,
  "exec": {
    "allowed_executables": ["/bin/echo"],
    "allow_shell": false
  }
}
```

## Policy Fields

### Sandbox

| Value | Description |
|-------|-------------|
| `seatbelt` | macOS Seatbelt sandbox (default) |
| `none` | No sandbox (requires `sandbox_unsafe_ack: true`) |

### Network

| Value | Description |
|-------|-------------|
| `disabled` | No network access (default) |
| `enabled` | Allow network (requires `network_unsafe_ack: true`) |

### Filesystem

```json
"fs": {
  "allowed_read": ["/tmp", "/usr/share"],
  "allowed_write": ["/tmp/output"],
  "working_dir": "/tmp"
}
```

- Paths must be absolute
- Cannot allow `/`, home directory, or system roots
- Write access requires `fs_write_unsafe_ack: true`

### Execution

```json
"exec": {
  "allowed_executables": ["/bin/cat", "/usr/bin/vim"],
  "allow_shell": false
}
```

- All paths must be absolute
- Shell execution disabled by default

## Acknowledgement Flags

Dangerous operations require explicit acknowledgement:

| Flag | Required When |
|------|---------------|
| `sandbox_unsafe_ack` | `sandbox: none` |
| `network_unsafe_ack` | `network: enabled` or unsandboxed |
| `fs_write_unsafe_ack` | Non-empty `allowed_write` |

## Best Practices

1. Use the most restrictive policy possible
2. Prefer `seatbelt` sandbox on macOS
3. Never allow shell unless absolutely necessary
4. Use specific executable paths, not directories
