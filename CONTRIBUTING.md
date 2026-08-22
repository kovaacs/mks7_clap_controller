# Contributing

## Setup

Install the pinned Rust toolchain and Apple Silicon target:

```sh
rustup toolchain install 1.98.0 --component clippy,rustfmt --target aarch64-apple-darwin
```

## Required Checks

Run these checks before opening a pull request:

```sh
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.98.0 test --workspace --all-targets --locked
```

Keep changes focused and update tests and documentation when behavior changes. Protocol changes must
distinguish facts from the MKS-7 manual from behavior verified against physical hardware.

## Bundle Validation

Changes to packaging, metadata, state, parameters, or CLAP behavior should also run the release
bundle checks used by CI. Install the pinned tools:

```sh
cargo +1.98.0 install --locked --version 0.9.2 --features cli cargo-about
cargo +1.98.0 install --locked --git https://github.com/free-audio/clap-validator.git --rev b2f1d9b79b1d264a5747f46707d72b1aa40a02ef clap-validator
```

Then generate the license report, package, and validate:

```sh
cargo +1.98.0 about generate --locked -o THIRD_PARTY_LICENSES.html about.hbs
cargo +1.98.0 run --locked -p xtask --release
bash scripts/validate-bundle.sh
```
