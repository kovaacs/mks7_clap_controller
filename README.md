# MKS-7 CLAP Controller

[![CI](https://github.com/kovaacs/mks7_clap_controller/actions/workflows/ci.yml/badge.svg)](https://github.com/kovaacs/mks7_clap_controller/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/kovaacs/mks7_clap_controller)](https://github.com/kovaacs/mks7_clap_controller/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

An Apple Silicon macOS CLAP plug-in bundle for editing the Roland MKS-7 from compatible CLAP hosts.
It exposes the MKS-7's synthesis parameters as automatable, modulatable controls and saves their
state with the host project. Bitwig Studio is the currently documented and tested host.

The bundle provides three note effects:

- `MKS-7 Melody Controller`
- `MKS-7 Chord Controller`
- `MKS-7 Bass Controller`

The plug-ins send parameter changes directly through CoreMIDI. They are controllers, not software
instruments: use Bitwig's HW Instrument after each controller for notes and audio return.

## Features

- Melody, Chord, and Bass synthesis controls, including the hardware-verified packed switches
- Chord four-voice/six-voice Whole Mode
- Configurable MIDI output and receive channel for each part
- Automation, modulation, remote-control pages, and project state restoration
- Rate-limited and coalesced MIDI output
- Best-effort note and MIDI event pass-through to the following device

There is no custom interface; Bitwig's parameter and remote-control panels are the front panel.
Rhythm mapping, program changes, hardware state reading, bulk tone writes, preset management, and
MIDI destination hot-plug refresh are not implemented.

## Build

Development and CI use Rust 1.98.0, pinned in `rust-toolchain.toml`. Rustup automatically installs
the required formatting and linting components and Apple Silicon target.

```sh
cargo test --workspace
cargo run -p xtask --release
```

The Apple Silicon bundle is written to `target/clap/MKS-7 Controller.clap`, targets macOS 11.0
or newer, and is ad-hoc signed. Use `cargo run -p xtask -- --debug` for a debug build.

Only macOS is supported because the MIDI transport uses CoreMIDI. Other targets are rejected at
compile time until they have a verified transport and packaging path.

## Install

Download the Apple Silicon macOS archive and checksum from
[GitHub Releases](https://github.com/kovaacs/mks7_clap_controller/releases), then verify it:

```sh
shasum -a 256 -c MKS-7-Controller-*-macOS-Apple-Silicon.zip.sha256
```

Place or symlink `MKS-7 Controller.clap` in:

```text
~/Library/Audio/Plug-Ins/CLAP/
```

Then rescan plug-ins in Bitwig.

The bundle is ad-hoc signed, not Developer ID signed or notarized. macOS may quarantine a downloaded
copy. Verify the bundle before removing quarantine:

```sh
codesign --verify --deep --strict --verbose=2 "MKS-7 Controller.clap"
xattr -d com.apple.quarantine "MKS-7 Controller.clap"
```

Only remove quarantine from a bundle you built yourself or downloaded from a release you trust.

## Use In Bitwig

1. Add the controller for the required MKS-7 part.
2. Add Bitwig's HW Instrument immediately after it.
3. In HW Instrument, select the physical MIDI output and the part's receive channel.
4. In the controller, select the same MIDI output and channel.
5. Move `VCF Cutoff` slowly and confirm that the hardware responds.

New instances have no MIDI output selected. Select or load the desired parameter values before
choosing an output if sending the initial zero-valued continuous parameters would be undesirable.

MIDI destinations are read when an instance is created. Reload the instance after connecting or
renaming a destination. Saved destinations are restored by CoreMIDI unique ID and fall back to
`None` if unavailable.

Each instance sends at most one successful message about every 25 ms. Melody, Chord, and Bass have
independent senders, so using all three can exceed 40 messages per second in total.

Note and MIDI events are forwarded unchanged when the host accepts them. CLAP does not provide a
retry path when the host's output event queue is full, so queue exhaustion can drop pass-through
events rather than interrupting real-time processing.

See [`docs/MKS-7_DEVELOPER_REFERENCE.md`](docs/MKS-7_DEVELOPER_REFERENCE.md) for the implemented MIDI
protocol and hardware-verified encodings.

## Contributing

Contributions are welcome. See [`CONTRIBUTING.md`](CONTRIBUTING.md) for setup, required checks, and
hardware-validation expectations.

## Disclaimer

This is an independent, unofficial project and is not affiliated with or endorsed by Roland or
Bitwig. Roland, MKS-7, Bitwig, and Bitwig Studio are trademarks of their respective owners.

## License

This project is available under the [MIT License](LICENSE). Dependency license texts are listed in
[`THIRD_PARTY_LICENSES.html`](THIRD_PARTY_LICENSES.html).
