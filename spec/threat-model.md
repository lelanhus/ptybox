# Threat Model

This document defines the security threat model for ptybox. It describes the scope of protection, trust boundaries, security controls, enforcement points, known limitations, and residual risks.

**Change control**: Changes to security controls, trust boundaries, or enforcement points must update this document, `spec/feature-list.json`, and `CHANGELOG.md`.

## Scope

### What ptybox protects against

- **Child process escaping policy restrictions**: The child process runs under a deny-by-default policy. Filesystem, network, executable, and environment variable access are restricted to explicitly allowlisted resources.
- **Unauthorized filesystem access**: Read and write access are limited to absolute-path allowlists. System roots (`/`, `/System`, `/Library`, `/Users`, `/private`, `/Volumes`) and the user's home directory are rejected as allowlist entries.
- **Unauthorized network access**: Network is disabled by default. Enabling it requires explicit acknowledgement. On macOS, Seatbelt enforces network restrictions at the OS level.
- **Resource exhaustion (DoS)**: Hard budgets limit runtime (`max_runtime_ms`), step count (`max_steps`), output volume (`max_output_bytes`), snapshot size (`max_snapshot_bytes`), and per-wait duration (`max_wait_ms`). Regex patterns are bounded by source length and compiled DFA size to prevent ReDoS.
- **Policy configuration errors**: Validation catches relative paths, overly broad allowlists, missing acknowledgements, dangerous environment variables, and policy version mismatches before execution begins.
- **Sandbox escape via environment variables**: Variables that enable library injection (`LD_PRELOAD`, `DYLD_INSERT_LIBRARIES`, etc.) are unconditionally blocked using case-insensitive matching.
- **Shell injection**: Shell execution is disabled by default. Shell detection resolves symlinks to prevent basename-based bypass.
- **Sandbox profile injection**: Seatbelt profile paths are validated against a character whitelist to prevent S-expression injection.

### What ptybox does NOT protect against

- **Kernel exploits**: If the kernel is compromised, all userspace isolation is meaningless. ptybox assumes a trusted kernel.
- **Hardware side channels**: Spectre, Meltdown, and similar attacks are outside scope.
- **Attacks on the host before ptybox runs**: If the host system is already compromised (e.g., a rootkit is installed), ptybox cannot provide meaningful isolation.
- **Supply chain attacks on ptybox itself**: Compromised dependencies or build toolchains are not detected at runtime. Mitigation is through standard dependency auditing and reproducible builds.
- **Privilege escalation via setuid/setgid binaries**: If an allowed executable has elevated privileges, ptybox does not prevent the child from using those privileges.
- **Denial of service against the host**: While budgets limit ptybox's own resource consumption, the child process can still consume CPU and memory up to OS limits (unless further constrained by cgroups or containers).

## Trust Boundaries

| Actor | Trust Level | Description |
|-------|-------------|-------------|
| **Policy author** | Trusted | Writes the policy JSON that defines security constraints. A malicious policy author can weaken all protections. |
| **Scenario author** | Semi-trusted | Writes step sequences (actions and assertions). All actions are validated against the policy before execution. A scenario author cannot exceed the privileges granted by the policy. |
| **Child process** | Untrusted | Runs under sandbox constraints. Cannot access resources outside the policy allowlists (when sandbox is enabled). |
| **Agent / LLM** | Untrusted | Drives the session via the NDJSON driver protocol. Every action is validated against the policy. Protocol version mismatches are rejected. |

### Trust boundary transitions

```
Policy Author (trusted)
    |
    v
+-------------------+
| Policy Validation  |  <-- Pre-run: all constraints checked
+-------------------+
    |
    v
Scenario Author (semi-trusted)
    |
    v
+-------------------+
| Action Validation  |  <-- Each action checked against policy
+-------------------+
    |
    v
+-------------------+
| Seatbelt Sandbox   |  <-- OS-level enforcement (macOS)
| or Container       |  <-- External enforcement (Linux)
+-------------------+
    |
    v
Child Process (untrusted)
```

## Security Controls

### Layer 1: Policy Validation

**Module**: `policy/mod.rs`
**Timing**: Pre-run (before the child process is spawned)

Policy validation is the first line of defense. It enforces deny-by-default semantics and catches configuration errors before any code executes.

Controls:
- **Deny-by-default**: Empty allowlists deny all access. Every privilege must be explicitly granted.
- **Absolute paths required**: All filesystem paths, executable paths, and working directories must be absolute. Relative paths are rejected.
- **Blocked system roots**: Allowlisting `/`, `/System`, `/Library`, `/Users`, `/private`, `/Volumes`, or the user's home directory is rejected with `E_POLICY_DENIED`.
- **Symlink validation**: Policy paths are checked for user-created symlinks. Well-known system symlinks (`/tmp`, `/var`, `/etc`, etc.) are exempted.
- **Path normalization**: `canonicalize_for_policy()` resolves `.` and `..` components lexically to prevent traversal attacks like `/allowed/../etc/shadow`.
- **Dangerous env var blocking**: `LD_PRELOAD`, `DYLD_INSERT_LIBRARIES`, `PYTHONPATH`, `IFS`, and similar variables are unconditionally blocked with case-insensitive matching.
- **Shell detection**: `is_shell_command()` blocks shell interpreters (`sh`, `bash`, `zsh`, etc.) and `.sh` scripts unless `allow_shell` is explicitly enabled. Resolves symlinks to prevent bypass via symlinked shells.
- **Write acknowledgements**: Non-empty `allowed_write` requires `fs_write_unsafe_ack: true`. Optional strict write mode (`fs_strict_write`) requires acknowledgement for any write activity.
- **Network acknowledgements**: Enabling network requires `network_unsafe_ack: true`. Disabling sandbox also requires network acknowledgement because network restrictions cannot be enforced without a sandbox.
- **Policy version check**: Rejects policies with unsupported `policy_version` values.
- **Env var set/allowlist consistency**: Environment variables in `set` must also appear in `allowlist`.
- **Artifacts directory validation**: Artifacts directory must be absolute and within `allowed_write` paths.

### Layer 2a: Seatbelt Sandbox (macOS)

**Module**: `policy/sandbox.rs`
**Timing**: Pre-run (profile generation) and runtime (OS enforcement)

On macOS, ptybox generates a Seatbelt profile and spawns the child process under `sandbox-exec`. The profile translates the policy into OS-level restrictions.

Controls:
- **Profile generation**: A `(deny default)` profile is generated from the policy. Only explicitly allowed resources are permitted.
- **Filesystem restrictions**: `file-read*` and `file-write*` rules are generated from `allowed_read` and `allowed_write` using `subpath` matching.
- **Network control**: `network-outbound` is allowed only when the policy enables network access.
- **Process execution limits**: `process-exec` rules allow only executables in the policy allowlist, using `literal` matching.
- **Profile path injection prevention**: `validate_seatbelt_path()` uses a character whitelist (alphanumeric, `-`, `_`, `.`, `/`, `@`, space) to prevent S-expression injection in profile strings.
- **Restrictive file permissions**: Sandbox profiles are written with mode `0600` to prevent other users from reading the sandbox rules.
- **Sandbox availability check**: `ensure_sandbox_available()` verifies that `sandbox-exec` works before attempting to use it. Failure produces `E_SANDBOX_UNAVAILABLE` rather than a silent fallback.

### Layer 2b: Container Isolation (Linux)

**Timing**: External to ptybox

On Linux, ptybox does not provide its own OS-level sandbox. The security boundary is provided by an external container (Docker, Podman, etc.). ptybox validates the policy but cannot enforce filesystem or network restrictions at the OS level.

Controls:
- **Policy validation**: All Layer 1 controls apply. The policy is validated even when sandbox is disabled.
- **Explicit acknowledgement**: Running without a sandbox requires `sandbox: "none"` and `sandbox_unsafe_ack: true`. This ensures the operator consciously accepts the reduced security posture.
- **Network acknowledgement**: When sandbox is disabled, `network_unsafe_ack: true` is required even if network is disabled, because network restrictions cannot be enforced.

### Layer 3: Resource Budgets

**Modules**: `runner/mod.rs`, `driver/mod.rs`
**Timing**: Runtime

Budgets prevent resource exhaustion from runaway processes, infinite loops, or adversarial inputs.

Controls:
- **`max_runtime_ms`**: Total wall-clock time for the run. Checked against a monotonic clock deadline.
- **`max_steps`**: Maximum number of scenario steps or driver actions.
- **`max_output_bytes`**: Combined transcript and terminal output budget. Checked after each observation.
- **`max_snapshot_bytes`**: Maximum serialized size of a single screen snapshot.
- **`max_wait_ms`**: Maximum duration for a single wait condition. Per-wait timeouts are capped to this value.
- **Regex pattern limits**: `compile_safe_regex()` enforces both source pattern length (`MAX_REGEX_PATTERN_LEN`) and compiled DFA size (`MAX_REGEX_SIZE = 1 MB`) to prevent ReDoS attacks.

### Layer 4: Process Group Control

**Module**: `session/mod.rs`
**Timing**: Runtime and post-run

Process group management ensures that all child processes (including forked descendants) are tracked and terminated.

Controls:
- **Process group signals**: `SIGTERM` and `SIGKILL` are sent to the process group (`killpg`), not just the child PID. This kills all descendants.
- **Graceful termination with escalation**: `terminate_process_group()` sends `SIGTERM`, waits up to a grace period, then sends `SIGKILL` if the process is still alive.
- **Drop cleanup**: The `Session::drop()` implementation performs best-effort cleanup (SIGTERM, 100ms wait, then SIGKILL) to prevent orphaned processes even when errors occur.
- **Explicit close**: `Session::close()` provides error-propagating shutdown for controlled termination.
- **Terminal size bounds**: Resize actions are bounded to `1..=500` for both rows and columns to prevent memory exhaustion from maliciously large terminal dimensions.

### Layer 5: Artifact Integrity

**Module**: `artifacts/mod.rs`
**Timing**: Post-run

Artifacts provide a verifiable record of the run for debugging and regression testing.

Controls:
- **Atomic writes**: JSON artifacts are written via a temp file + rename pattern to prevent partial writes from leaving corrupt files on interruption.
- **Checksums**: `checksums.json` records 64-bit checksums for all artifact files, enabling integrity verification.
- **Overwrite protection**: By default, writing to an existing artifacts directory is rejected unless `overwrite: true` is set.

## Enforcement Point Table

| Control | Enforcement Point | Module | Timing |
|---------|-------------------|--------|--------|
| Policy version check | `validate_policy_version()` | `policy/mod.rs` | Pre-run |
| Sandbox availability | `ensure_sandbox_available()` | `policy/sandbox.rs` | Pre-run |
| Sandbox acknowledgement | `validate_sandbox_mode()` | `policy/mod.rs` | Pre-run |
| Executable allowlist | `EffectivePolicy::validate_run_config()` | `policy/mod.rs` | Pre-run |
| Absolute path enforcement | `validate_fs_policy()` | `policy/mod.rs` | Pre-run |
| Blocked system roots | `disallowed_allowlist_reason()` | `policy/mod.rs` | Pre-run |
| Symlink detection | `validate_path_not_symlink()` | `policy/mod.rs` | Pre-run |
| Path normalization | `canonicalize_for_policy()` | `policy/mod.rs` | Pre-run |
| Env var blocking | `apply_env_policy()` | `policy/mod.rs` | Pre-run |
| Shell detection | `is_shell_command()` | `policy/mod.rs` | Pre-run |
| Network acknowledgement | `validate_network_policy()` | `policy/mod.rs` | Pre-run |
| Write acknowledgement | `validate_write_access()` | `policy/mod.rs` | Pre-run |
| Artifacts dir validation | `validate_artifacts_dir()` | `policy/mod.rs` | Pre-run |
| Seatbelt path sanitization | `validate_seatbelt_path()` | `policy/sandbox.rs` | Pre-run |
| Sandbox profile generation | `write_profile()` | `policy/sandbox.rs` | Pre-run |
| Runtime budget enforcement | Deadline-based checks | `runner/mod.rs`, `driver/mod.rs` | Runtime |
| Step budget enforcement | Sequence counter checks | `runner/mod.rs`, `driver/mod.rs` | Runtime |
| Output budget enforcement | Cumulative byte tracking | `runner/mod.rs`, `driver/mod.rs` | Runtime |
| Snapshot size enforcement | Per-observation size check | `runner/mod.rs`, `driver/mod.rs` | Runtime |
| Wait timeout enforcement | `max_wait_ms` cap | `runner/mod.rs`, `driver/mod.rs` | Runtime |
| Regex pattern limits | `compile_safe_regex()` | `runner/mod.rs` | Runtime |
| Terminal size bounds | Resize action validation | `session/mod.rs` | Runtime |
| Protocol version validation | Request version check | `driver/mod.rs` | Runtime |
| Process group termination | `terminate_process_group()` | `session/mod.rs` | Post-run |
| Artifact atomic writes | `atomic_write()` | `artifacts/mod.rs` | Post-run |
| Artifact checksums | `record_checksum()` | `artifacts/mod.rs` | Post-run |
| Drop cleanup | `cleanup_process_best_effort()` | `session/mod.rs` | Post-run |

## Known Limitations

### TOCTOU symlink race

`validate_path_not_symlink()` checks are vulnerable to Time-of-Check-Time-of-Use (TOCTOU) race conditions. An attacker could:

1. Create a regular file at an allowed path.
2. Wait for the symlink validation to pass.
3. Replace the file with a symlink to a sensitive location.

**Mitigation**: The Seatbelt sandbox provides runtime protection against this attack because it enforces path restrictions at the kernel level. When the sandbox is disabled (`--no-sandbox --ack-unsafe-sandbox`), this TOCTOU race becomes exploitable. This limitation is inherent to the Unix filesystem model and cannot be fully mitigated without OS-level support.

### macOS version dependency

Seatbelt behavior may vary across macOS versions. ptybox is tested on macOS 14 and later. Older macOS versions may have different sandbox capabilities or restrictions.

### No Linux host sandbox

On Linux without containers, the policy is validated but not enforced at the OS level. The child process can access any resource the parent process can access, regardless of what the policy specifies. Operators must rely on container isolation (Docker, Podman, etc.) for enforcement on Linux.

### No policy signing

Policies are not cryptographically signed. Integrity depends on file system permissions and the trustworthiness of the policy author. A compromised policy file can weaken all protections.

### Seatbelt IPC limits

Seatbelt does not restrict all IPC mechanisms. Some Mach ports, XPC connections, and other macOS-specific IPC channels may remain accessible to the child process even under sandbox restrictions.

### Lexical path canonicalization

`canonicalize_for_policy()` performs lexical normalization only (resolving `.` and `..` components). It does not follow symlinks or resolve mount points. This means:

- Mount point tricks (e.g., bind mounts that map restricted paths into allowed paths) are not caught by policy validation alone.
- Symlink-based path aliasing is caught by `validate_path_not_symlink()` but subject to the TOCTOU limitation described above.

The Seatbelt sandbox resolves paths at the kernel level, providing defense-in-depth against these attacks on macOS.

### Environment variable scope

While dangerous environment variables are blocked, the blocklist is not exhaustive. Application-specific environment variables that influence behavior (e.g., `GIT_DIR`, `npm_config_*`) are not blocked because they are context-dependent and cannot be universally classified as dangerous.

### No cgroup-based resource limits

ptybox's resource budgets are enforced by polling (checking elapsed time, counting bytes). They do not use cgroups or other OS-level resource limiting mechanisms. This means:

- A child process can consume unbounded CPU and memory between budget checks.
- Budget enforcement depends on the runner loop executing frequently enough to catch violations.

Container-based deployments can layer cgroup limits on top of ptybox's budgets for stronger resource isolation.

## Residual Risks

After all controls are applied, the following risks remain:

1. **Kernel vulnerabilities**: Any kernel exploit can bypass all userspace isolation, including both Seatbelt and container boundaries.
2. **Seatbelt bypass**: Undiscovered vulnerabilities in the Seatbelt sandbox implementation could allow escape. This risk is mitigated by Apple's ongoing security updates.
3. **Resource consumption between budget checks**: A child process can briefly consume excessive resources between budget enforcement polling intervals.
4. **IPC-based information leakage**: Mach ports and other IPC mechanisms not fully restricted by Seatbelt could be used for side-channel communication or data exfiltration.
5. **Policy misconfiguration**: An overly permissive policy (while still passing validation) could grant the child process more access than intended. ptybox rejects the most common misconfigurations (broad roots, system paths) but cannot prevent all forms of over-permissioning.
6. **Time-based side channels**: The child process can observe timing information through system clocks and scheduling behavior, which could leak information about the host system.
7. **Filesystem race conditions**: The TOCTOU gap in symlink validation is exploitable when running without the Seatbelt sandbox.
8. **Unsandboxed Linux deployments**: If an operator runs ptybox on Linux without container isolation, policy restrictions are advisory only. The `sandbox_unsafe_ack` flag exists to ensure this is a conscious decision.
