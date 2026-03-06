# Threat Model

For a comprehensive analysis of ptybox's security architecture, trust boundaries, and known limitations, see the [full threat model specification](../../../spec/threat-model.md).

This guide provides a user-facing overview of what ptybox protects you from, how its security layers work, and what to be aware of when deploying it.

## What ptybox Protects You From

ptybox is designed to safely run untrusted terminal applications under strict constraints. It protects against the most common risks when automating TUI interactions:

- **Unauthorized file access**: The child process can only read and write files you explicitly allow. Broad paths like `/` or your home directory are rejected.
- **Unauthorized network access**: Network is off by default. Turning it on requires an explicit acknowledgement flag.
- **Runaway processes**: Hard budgets prevent the child from running forever, producing unlimited output, or consuming unbounded resources.
- **Shell injection**: Shell interpreters are blocked by default. Direct executable invocation is required.
- **Environment variable attacks**: Variables known to enable library injection (such as `LD_PRELOAD` and `DYLD_INSERT_LIBRARIES`) are unconditionally blocked.
- **Policy misconfiguration**: Common mistakes (relative paths, overly broad allowlists, missing acknowledgements) are caught before execution begins.

## What ptybox Does NOT Protect Against

ptybox operates at the application level. It cannot defend against:

- Kernel exploits or hardware side-channel attacks
- A compromised host system (rootkits, backdoors)
- Supply chain attacks on ptybox's own dependencies
- Privilege escalation through setuid/setgid binaries that happen to be in the executable allowlist

For maximum isolation, combine ptybox with container-based deployment (Docker, Podman) or run it on a dedicated host.

## Trust Boundaries

ptybox defines four trust levels. Each boundary has specific validation and enforcement mechanisms.

```
+-------------------------------------------------------+
|                                                       |
|  Policy Author (trusted)                              |
|  Defines what the child process is allowed to do.     |
|  A malicious policy weakens all protections.          |
|                                                       |
+---------------------------+---------------------------+
                            |
                            v
+---------------------------+---------------------------+
|                                                       |
|  Scenario Author (semi-trusted)                       |
|  Writes action sequences. Every action is checked     |
|  against the policy before execution.                 |
|                                                       |
+---------------------------+---------------------------+
                            |
                            v
+---------------------------+---------------------------+
|                                                       |
|  Sandbox / Container Boundary                         |
|  OS-level enforcement (Seatbelt on macOS,             |
|  container on Linux). This is the hard boundary.      |
|                                                       |
+---------------------------+---------------------------+
                            |
                            v
+---------------------------+---------------------------+
|                                                       |
|  Child Process / Agent (untrusted)                    |
|  Runs under all constraints. Cannot access            |
|  resources outside the policy allowlists.             |
|                                                       |
+-------------------------------------------------------+
```

The key insight: **the policy author is the root of trust**. If you do not trust the policy, you cannot trust the run.

## Security Layers

ptybox applies security in four distinct layers. Each layer adds defense-in-depth so that a failure in one layer does not compromise the entire system.

### Layer 1: Policy Validation (Pre-run)

Before the child process is spawned, ptybox validates the entire policy:

- All paths must be absolute (no relative path tricks).
- System-critical directories are rejected as allowlist entries.
- Symlinks in policy paths are detected and rejected.
- Dangerous environment variables are blocked regardless of the allowlist.
- Shell interpreters are blocked unless explicitly enabled.
- Missing acknowledgement flags cause immediate failure.

If any validation fails, ptybox exits with error code `2` (`E_POLICY_DENIED`) and a structured error message explaining exactly what went wrong.

### Layer 2: OS-Level Sandbox (Runtime)

On macOS, ptybox generates a Seatbelt sandbox profile from the policy and runs the child under `sandbox-exec`. The sandbox enforces restrictions at the kernel level:

- File reads and writes are limited to allowed paths.
- Network access is blocked unless enabled.
- Only allowed executables can be run.

On Linux, ptybox relies on an external container to provide the security boundary. The policy is validated but not enforced at the OS level.

### Layer 3: Resource Budgets (Runtime)

Hard limits prevent resource exhaustion:

- **Runtime**: Total wall-clock time for the run.
- **Steps**: Maximum number of actions.
- **Output**: Combined transcript and terminal output size.
- **Snapshots**: Maximum size of a single screen capture.
- **Wait duration**: Per-wait timeout cap.
- **Regex complexity**: Pattern length and compiled size are bounded to prevent ReDoS.

### Layer 4: Process Group Control (Post-run)

When a run ends (normally or due to error), ptybox terminates the entire process group:

1. Sends `SIGTERM` to all processes in the group.
2. Waits for graceful shutdown.
3. Sends `SIGKILL` if processes are still alive.

This prevents orphaned child processes from persisting after the run completes.

## When to Use Sandbox vs Containers

| Deployment | Sandbox | Container | Recommendation |
|-----------|---------|-----------|----------------|
| macOS development | Seatbelt (default) | Not needed | Use the default Seatbelt sandbox. |
| macOS CI | Seatbelt (default) | Optional | Seatbelt provides sufficient isolation for most CI workloads. |
| Linux CI | Not available | Required | Run ptybox inside a container. Use `sandbox: "none"` with `sandbox_unsafe_ack: true`. |
| High-security | Seatbelt | Recommended | Layer both for defense-in-depth. |
| Agent/LLM orchestration | Depends on OS | Strongly recommended | Untrusted agents should always run inside containers. |

## Known Limitations Users Should Be Aware Of

### Symlink race condition (TOCTOU)

ptybox checks for symlinks in policy paths before the run starts, but a sophisticated attacker could replace a file with a symlink after the check passes. The Seatbelt sandbox protects against this at runtime. Without the sandbox, this race is exploitable.

**Action**: Always use the sandbox on macOS. On Linux, use containers.

### No Linux host sandbox

On Linux without a container, policy restrictions are validated but not enforced by the OS. The child process can access any resource the parent can access.

**Action**: Always run ptybox inside a container on Linux.

### Unsigned policies

Policies are plain JSON files without cryptographic signatures. Anyone with write access to the policy file can weaken security constraints.

**Action**: Protect policy files with appropriate file system permissions. In CI, generate policies from a trusted source.

### IPC not fully restricted

On macOS, some inter-process communication mechanisms (Mach ports, XPC) are not fully restricted by Seatbelt. A child process could potentially communicate with other processes on the system through these channels.

**Action**: For high-security workloads, combine ptybox with container isolation to limit the IPC surface.

### Resource limits are polling-based

ptybox checks budgets during its event loop, not via OS-level resource limits. A child process can briefly exceed limits between checks.

**Action**: For strict resource control, layer cgroup limits (via containers) on top of ptybox's budgets.

## Further Reading

- [Sandbox Security](sandbox.md) -- detailed guide on sandbox configuration
- [Container Setup](containers.md) -- running ptybox in containers
- [Policies](policies.md) -- policy configuration reference
- [Full Threat Model Specification](../../../spec/threat-model.md) -- complete technical analysis with code-level enforcement details
