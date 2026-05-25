# duty-core

Owns shared duty data models and pure parsers.

## Rules

- Keep this crate free of process execution, filesystem IO, network IO, and
  terminal rendering.
- Put reusable queue/report data shapes in `model.rs`.
- Put parser functions that can be tested from static strings in focused
  modules such as `plain.rs`.
- Tests live in `tests/` and should not require `gh`, network access, or a
  writable repository checkout.

## Commands

```bash
cargo test --locked -p duty-core
```

