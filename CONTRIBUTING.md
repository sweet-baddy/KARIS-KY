# Contributing

Thanks for contributing to karis-ky. Changes to the escrow contract should
preserve Soroban storage compatibility and include focused tests for behavioral
changes.

## Setup

- Install stable Rust with `rustup` (Rust 1.70 or newer).
- For WASM builds, install the target with `rustup target add wasm32v1-none`.
- Soroban / Stellar CLI is optional for local contract deployment and
  interaction.

From the repository root, build the workspace:

```bash
cargo build
```

## Tests and checks

Run the full workspace test suite before submitting:

```bash
cargo test
```

For a faster escrow-only test run:

```bash
cargo test -p karis_ky_escrow
```

CI also checks formatting and lint:

```bash
cargo fmt -p karis_ky_escrow -- --check
cargo clippy -p karis_ky_escrow -- -D warnings
```

Add or update tests alongside contract behavior changes. For documentation-only
changes, check that edited links point to existing files and that indexes remain
consistent.

## Pull requests

- Keep durable user and developer documentation in `README.md` or the relevant
  `docs/` section.
- Put implementation summaries, completion reports, and other PR-specific
  summaries in the pull request description. Do not add them as new repository
  root Markdown files.
- Include the motivation, behavior change, relevant tests, and any operational
  considerations in the pull request description.