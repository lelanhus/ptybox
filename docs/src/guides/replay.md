# Replay Guide

`ptybox replay` re-runs the scenario captured in an artifacts directory and compares the replay run against the original bundle.

## 1) Record artifacts

```bash
ptybox run --json --scenario ./scenario.yaml --artifacts ./artifacts --overwrite
```

Required baseline files are generated in `./artifacts` (for example `scenario.json`, `policy.json`, `run.json`, `snapshots/`, `transcript.log`).

## 2) Replay deterministically

```bash
ptybox replay --json --artifacts ./artifacts
```

- exit `0`: replay matched
- exit `11`: replay mismatch (`E_REPLAY_MISMATCH`)

On each replay, `ptybox` writes a `replay-<run_id>/` folder under the original artifacts directory with:

- `run.json` for the replay execution
- `replay.json` summary
- `diff.json` when mismatch occurs

## Normalization controls

Replay can ignore known nondeterministic fields.

### Strict

```bash
ptybox replay --json --artifacts ./artifacts --strict
```

### Explicit filters

```bash
ptybox replay --json --artifacts ./artifacts \
  --normalize run_id \
  --normalize session_id
```

Supported `--normalize` values:

- `all`
- `none`
- `snapshot_id`
- `run_id`
- `run_timestamps`
- `step_timestamps`
- `observation_timestamp`
- `session_id`

### Explain resolved replay settings

```bash
ptybox replay --json --artifacts ./artifacts --explain
```

## Integrity gates

Require event/checksum files during replay:

```bash
ptybox replay --json --artifacts ./artifacts --require-events --require-checksums
```

## Replay report

Read the most recent replay summary:

```bash
ptybox replay-report --json --artifacts ./artifacts
```
