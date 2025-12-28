# Error Codes

ptybox uses stable exit codes and error codes for programmatic handling.

## Exit Codes

| Code | Name | Description |
|------|------|-------------|
| 0 | Success | Run completed successfully |
| 2 | E_POLICY_DENIED | Policy validation failed |
| 3 | E_SANDBOX_UNAVAILABLE | Sandbox backend not available |
| 4 | E_TIMEOUT | Timeout or budget exceeded |
| 5 | E_ASSERTION_FAILED | Assertion did not pass |
| 6 | E_PROCESS_EXIT | Target process exited non-zero |
| 7 | E_TERMINAL_PARSE | Terminal output parsing failed |
| 8 | E_PROTOCOL_VERSION | Incompatible protocol version |
| 9 | E_PROTOCOL | Malformed protocol message |
| 10 | E_IO | I/O operation failed |
| 11 | E_REPLAY_MISMATCH | Replay comparison failed |
| 12 | E_CLI_INVALID_ARG | Invalid CLI argument |

## Error Details

### E_POLICY_DENIED (2)

Policy validation failed before execution.

**Common causes:**
- Executable not in `allowed_executables`
- Path is relative (must be absolute)
- Forbidden path (/, $HOME, /System)
- Missing acknowledgement flag
- Shell execution when `allow_shell: false`

**Resolution:** Update policy to allow the operation or add required ack flags.

### E_SANDBOX_UNAVAILABLE (3)

Sandbox backend cannot be used.

**Common causes:**
- Running on Linux without container isolation
- Seatbelt not available on macOS

**Resolution:** Use `sandbox: none` with appropriate container isolation.

### E_TIMEOUT (4)

Operation exceeded time limit.

**Common causes:**
- Step timeout exceeded
- Total runtime budget exceeded
- Wait condition never satisfied

**Resolution:** Increase timeout or fix the application under test.

### E_ASSERTION_FAILED (5)

An assertion did not pass.

**Common causes:**
- Expected text not on screen
- Cursor not at expected position
- Regex pattern didn't match

**Resolution:** Check assertion expectations match actual output.

### E_PROCESS_EXIT (6)

Target process exited with non-zero code.

**Resolution:** Check the target application for errors.

### E_TERMINAL_PARSE (7)

Failed to parse terminal output.

**Common causes:**
- Invalid UTF-8 in output
- Unsupported escape sequences

**Resolution:** Check target application output encoding.

### E_PROTOCOL_VERSION (8)

Protocol version mismatch.

**Resolution:** Update client or server to compatible versions.

### E_PROTOCOL (9)

Malformed protocol message.

**Common causes:**
- Invalid JSON
- Missing required fields
- Unknown action type

**Resolution:** Check message format against protocol spec.

### E_IO (10)

I/O operation failed.

**Common causes:**
- Permission denied
- Disk full
- File not found

**Resolution:** Check file permissions and paths.

### E_REPLAY_MISMATCH (11)

Replay comparison found differences.

**Resolution:** Review differences, update baseline, or adjust normalization.

### E_CLI_INVALID_ARG (12)

Invalid command-line argument.

**Resolution:** Check `ptybox --help` for correct usage.
