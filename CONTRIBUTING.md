# Contributing

Contributions that improve reliability, hardware support, host compatibility, documentation, or
the editing experience are welcome. Keep changes focused and explain any protocol or hardware
assumptions.

## Before You Start

- Search existing issues and pull requests for related work.
- Open a feature request before making a substantial behavior, protocol, or compatibility change.
- Do not include private project data, credentials, or unrelated host logs in issues or commits.

## Development Setup

Install [rustup](https://rustup.rs/), then enter the repository. Rustup automatically installs the
toolchain, components, and Apple Silicon target pinned in `rust-toolchain.toml` when Cargo runs.
You can initialize it explicitly with:

```sh
rustup show active-toolchain
```

## Required Checks

Install the pinned dependency auditor once, then run the required checks before opening a pull
request:

```sh
cargo install --locked --version 0.22.2 cargo-audit
```

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo audit
```

## Making Changes

- Create a focused branch from the latest `main`.
- Follow the existing Rust style and avoid unrelated formatting changes.
- Add or update tests for behavior that does not require physical hardware.
- Preserve serialized state and plug-in identity when compatibility is required.
- Update the README and developer reference when user-visible or protocol behavior changes.
- Distinguish facts from the MKS-7 manual from behavior verified against physical hardware.

## Bundle Validation

Changes to packaging, metadata, state, parameters, or CLAP behavior should also run the release
bundle checks used by CI. Install the pinned tools:

```sh
cargo install --locked --version 0.9.2 --features cli cargo-about
cargo install --locked --git https://github.com/free-audio/clap-validator.git --rev b2f1d9b79b1d264a5747f46707d72b1aa40a02ef clap-validator
```

Then generate the license report, package, and validate:

```sh
cargo about generate --locked -o THIRD_PARTY_LICENSES.html about.hbs
cargo run --locked -p xtask --release
scripts/validate-bundle.sh
```

## Pull Requests

Pull requests must pass both required CI jobs:

- `Tests`
- `Build`

Describe what changed, why it changed, and how it was tested. Include the macOS and Bitwig versions,
MKS-7 part, MIDI interface, and hardware observations when relevant. State clearly when physical
hardware validation was not performed.

The repository uses squash merging. Write a concise pull request title suitable for the resulting
commit.
