## Summary

Describe what changed and why.

## Validation

- [ ] Formatting passes (`cargo fmt --all -- --check`)
- [ ] Clippy passes (`cargo clippy --workspace --all-targets --locked -- -D warnings`)
- [ ] Tests pass (`cargo test --workspace --all-targets --locked`)
- [ ] Bundle validation passes (`scripts/validate-bundle.sh`), or packaging and CLAP behavior are not affected
- [ ] Physical MKS-7 behavior was tested, or hardware behavior is not affected
- [ ] Documentation and third-party license output were updated if needed

## Hardware Notes

List the MKS-7 part, MIDI interface, host, and any observed results. State explicitly if hardware testing was not performed.
