# Replay & Normalization

Record runs and compare against baselines for regression testing.

## Recording a Baseline

```bash
ptybox run --json --scenario test.yaml --artifacts ./baseline
```

## Replaying Against Baseline

```bash
ptybox replay --baseline ./baseline --artifacts ./current
```

Exit code 0 means match, exit code 11 (`E_REPLAY_MISMATCH`) means difference.

## Normalization

Some outputs are non-deterministic (timestamps, PIDs). Use normalization filters:

```bash
ptybox replay --baseline ./baseline --artifacts ./current \
  --normalize timestamps \
  --normalize pids
```

### Available Filters

| Filter | Description |
|--------|-------------|
| `none` | Strict comparison, no normalization |
| `all` | Apply all normalizations |
| `timestamps` | Normalize time-like patterns |
| `pids` | Normalize process IDs |
| `snapshot_id` | Ignore snapshot IDs |
| `run_id` | Ignore run IDs |

### Policy-Based Normalization

Define normalization in the policy:

```json
{
  "replay": {
    "normalization": {
      "defaults": ["timestamps", "pids"],
      "strict": false,
      "rules": [
        { "pattern": "\\d{4}-\\d{2}-\\d{2}", "replacement": "DATE" }
      ]
    }
  }
}
```

## Replay Report

Generate an HTML diff report:

```bash
ptybox replay-report --baseline ./baseline --output diff.html
```

## CI Integration

```yaml
# GitHub Actions example
- name: Run tests
  run: ptybox run --scenario test.yaml --artifacts ./current

- name: Compare to baseline
  run: ptybox replay --baseline ./baseline --artifacts ./current
```

## Strict Mode

Disable all normalization for exact comparison:

```bash
ptybox replay --baseline ./baseline --artifacts ./current --strict
```
