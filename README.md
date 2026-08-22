# MKS-7 CLAP Controller

A macOS CLAP plug-in bundle for editing the Roland MKS-7 from compatible CLAP hosts. It exposes the
MKS-7's synthesis parameters as automatable, modulatable controls and saves their state with the
host project. Bitwig Studio is the currently documented and tested host.

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
- Note and MIDI event pass-through to the following device

There is no custom interface; Bitwig's parameter and remote-control panels are the front panel.
Rhythm mapping, program changes, hardware state reading, bulk tone writes, preset management, and
MIDI destination hot-plug refresh are not implemented.

## Build

Rust 1.85 or newer is required.

```sh
cargo test --workspace
cargo run -p xtask --release
```

The bundle is written to `target/clap/MKS-7 CLAP Controller.clap` and ad-hoc signed. Use
`cargo run -p xtask -- --debug` for a debug build.

Only macOS is supported because the MIDI transport uses CoreMIDI. Other targets are rejected at
compile time until they have a verified transport and packaging path.

## Install

Place or symlink `MKS-7 CLAP Controller.clap` in:

```text
~/Library/Audio/Plug-Ins/CLAP/
```

Then rescan plug-ins in Bitwig.

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

See [`docs/MKS-7_DEVELOPER_REFERENCE.md`](docs/MKS-7_DEVELOPER_REFERENCE.md) for the implemented MIDI
protocol and hardware-verified encodings.
