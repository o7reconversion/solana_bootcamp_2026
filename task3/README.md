# Task 3 Changes

Summary of changes made in `task3/blueshift_anchor_escrow`:

- Pinned dependency versions in `Cargo.lock` to avoid the `edition2024` requirement from `constant_time_eq`.
  - `blake3` is locked to `1.5.5`.
  - `constant_time_eq` is locked to `0.3.1`.

Why:
- `constant_time_eq >=0.4.2` requires Cargo support for `edition2024`.
- The Solana SBF build toolchain in this environment uses an older Cargo, so the build fails unless these versions are pinned.

Files touched:
- `solana_bootcamp_2026/task3/blueshift_anchor_escrow/Cargo.lock`
