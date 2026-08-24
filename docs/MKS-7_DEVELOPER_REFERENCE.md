# MKS-7 Protocol Reference

Protocol facts used by this project, normalized from pages 25-30 of the Roland MKS-7 owner's manual
and corrected where noted through hardware testing. The implemented receive behavior was also
cross-checked against the reverse-engineered main ROM source at
[`MKS7main.asm` revision `42f7577`](https://github.com/handsthatstrike/RolandMKS7/blob/42f7577f60265e42867dfdda0c83922650bc846a/MKS7main.asm).
Do not infer undocumented fields or bit masks.

The ROM source confirms message parsing and parameter routing, but its comments are reverse-engineering
notes rather than Roland documentation. Hardware observations remain authoritative for user-facing
switch meanings and audible behavior.

## Parts And Channels

| Part | Voices | Factory channel |
|---|---:|---:|
| Melody | 2 | 1 |
| Bass | 1 | 2 |
| Chord | 4 | 3 |
| Rhythm | drum voices | 10 |

Each part can use MIDI channel 1-16. MIDI messages and SysEx encode this user-facing channel as a
zero-based nibble (`channel - 1`). Do not assume factory channels when a user has changed them.

## Parameter Writes

A single-parameter write is:

```text
F0 41 32 uu pp vv F7
```

The firmware accepts manufacturer `41` and operation codes `30`, `31`, and `32` at `L0211-L022A`.
For operation `32`, it saves `pp`, requires it to be below `0x12`, and marks `vv` in the corresponding
parameter slot at `L0200-L020F`. The main loop finds and dispatches that slot through `L1178-L1189`
and `L10F8-L1177`. Thus the short single-parameter form above is sufficient; it does not require
padding preceding parameter slots.

| Byte | Meaning |
|---|---|
| `41` | Roland manufacturer ID |
| `32` | Tone parameter change |
| `uu` | Zero-based MIDI channel, `00`-`0F` |
| `pp` | Parameter number, `00`-`11` |
| `vv` | Value, `00`-`7F` |

Example: Melody channel 1, VCF cutoff 96:

```text
F0 41 32 00 05 60 F7
```

## Melody And Chord

| # | Parameter |
|---:|---|
| 0 | LFO Rate |
| 1 | LFO Delay |
| 2 | DCO LFO |
| 3 | DCO PWM |
| 4 | Dynamics switches |
| 5 | VCF Cutoff |
| 6 | VCF Resonance |
| 7 | VCF ENV |
| 8 | VCF LFO |
| 9 | VCF KYBD |
| 10 | VCA Level |
| 11 | Attack |
| 12 | Decay |
| 13 | Sustain |
| 14 | Release |
| 15 | Sub Level |
| 16 | Oscillator switches |
| 17 | Mode switches |

Parameters other than 4, 15, 16, and 17 use values `0..127`.

The Melody/Chord routing table is stored at `L000B` and applied by `L10F8-L1177`. Parameters 4,
15, 16, and 17 take dedicated packed-value paths.

### Parameter 15

The Sub Level is a four-step control implemented by a 2-bit D/A converter. The hardware uses only
bits 5 and 6 of the received value:

| Value | Sub Level |
|---:|---:|
| `0..31` | 0 (off) |
| `32..63` | 1 |
| `64..95` | 2 |
| `96..127` | 3 |

Send the canonical values `0`, `32`, `64`, and `96`.

### Parameter 4

Hardware-verified dynamics switches:

| Bit | Function when set |
|---:|---|
| 5 | VCA Dynamics on |
| 6 | VCF Dynamics on |

Bits 0-4 are ignored. Compose the complete byte using `00`, `20`, `40`, or `60`.

### Parameter 16

Verified against physical hardware and the working `MKS-7.amxd` device:

| Bit | Function |
|---:|---|
| 0 | 16' range when set |
| 1 | 8' range when set |
| 2 | 4' range when set |
| 3 | Pulse on when set |
| 4 | Saw on when set |
| 5 | Chorus off when set |

Range is one-hot. Always compose and send the complete byte so changing one control preserves the
others.

### Parameter 17

| Bit | Function |
|---:|---|
| 0 | PWM Manual when set, LFO when clear |
| 1 | VCF ENV negative when set, positive when clear |
| 2 | VCA Gate when set, ENV when clear |
| 3 | Ignored |
| 4 | HPF off when set, on when clear |
| 5 | Melody Noise on when set; unused for Chord |

These encodings are hardware-verified. Bit 3 produced no measurable response; bit 4 selects the two
actual HPF states. Always compose the complete byte.

## Bass

Bass implements parameters 3, 5, 6, 7, 9-14 with the same names as the Melody/Chord table. Bass
parameter 16 bit 4 selects the waveform: clear is Pulse and set is Saw. This behavior is
hardware-verified. Other bits and parameter 17 are ignored. The firmware's Bass dispatch table at
`L10E0`, used by `L1050-L10DF`, confirms these implemented parameter numbers and the parameter 16
special case.

## Whole Mode

Chord Control Change 121 selects voice mode:

```text
value 0   -> 4 voices
value 127 -> 6 voices
```

This was verified against hardware and `MKS-7.amxd`. CC 127, suggested by an earlier reading of the
manual scan, did not switch the physical unit. In six-voice mode, Melody does not sound independently
and the Chord channel controls the combined synth. Firmware status decoding assigns the Chord channel
to target bit 1 at `L0050-L007D`; the CC 121 handler tests that bit and accepts values 0 and 127 at
`L0171-L0192`.

## Transport Constraints

The controller state is authoritative because the project has no verified hardware state-dump path.
Complete-state restoration and continuous edits must remain rate-limited, and ignored bits in packed
parameters must remain clear.
