#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParameterDefinition {
    pub id: u32,
    pub protocol_number: u8,
    pub name: &'static [u8],
    pub module: &'static [u8],
}

pub const MELODY_PARAMETERS: [ParameterDefinition; 15] = [
    ParameterDefinition {
        id: 0,
        protocol_number: 0,
        name: b"LFO Rate",
        module: b"LFO",
    },
    ParameterDefinition {
        id: 1,
        protocol_number: 1,
        name: b"LFO Delay",
        module: b"LFO",
    },
    ParameterDefinition {
        id: 2,
        protocol_number: 2,
        name: b"DCO LFO",
        module: b"DCO",
    },
    ParameterDefinition {
        id: 3,
        protocol_number: 3,
        name: b"DCO PWM",
        module: b"DCO",
    },
    ParameterDefinition {
        id: 4,
        protocol_number: 5,
        name: b"VCF Cutoff",
        module: b"VCF",
    },
    ParameterDefinition {
        id: 5,
        protocol_number: 6,
        name: b"VCF Resonance",
        module: b"VCF",
    },
    ParameterDefinition {
        id: 6,
        protocol_number: 7,
        name: b"VCF ENV",
        module: b"VCF",
    },
    ParameterDefinition {
        id: 7,
        protocol_number: 8,
        name: b"VCF LFO",
        module: b"VCF",
    },
    ParameterDefinition {
        id: 8,
        protocol_number: 9,
        name: b"VCF KYBD",
        module: b"VCF",
    },
    ParameterDefinition {
        id: 9,
        protocol_number: 10,
        name: b"VCA Level",
        module: b"VCA",
    },
    ParameterDefinition {
        id: 10,
        protocol_number: 11,
        name: b"Attack",
        module: b"Envelope",
    },
    ParameterDefinition {
        id: 11,
        protocol_number: 12,
        name: b"Decay",
        module: b"Envelope",
    },
    ParameterDefinition {
        id: 12,
        protocol_number: 13,
        name: b"Sustain",
        module: b"Envelope",
    },
    ParameterDefinition {
        id: 13,
        protocol_number: 14,
        name: b"Release",
        module: b"Envelope",
    },
    ParameterDefinition {
        id: 14,
        protocol_number: 15,
        name: b"Sub Level",
        module: b"DCO",
    },
];

pub const CHORD_PARAMETERS: [ParameterDefinition; 15] = MELODY_PARAMETERS;

pub const BASS_PARAMETERS: [ParameterDefinition; 10] = [
    ParameterDefinition {
        id: 0,
        protocol_number: 3,
        name: b"DCO PWM",
        module: b"DCO",
    },
    ParameterDefinition {
        id: 1,
        protocol_number: 5,
        name: b"VCF Cutoff",
        module: b"VCF",
    },
    ParameterDefinition {
        id: 2,
        protocol_number: 6,
        name: b"VCF Resonance",
        module: b"VCF",
    },
    ParameterDefinition {
        id: 3,
        protocol_number: 7,
        name: b"VCF ENV",
        module: b"VCF",
    },
    ParameterDefinition {
        id: 4,
        protocol_number: 9,
        name: b"VCF KYBD",
        module: b"VCF",
    },
    ParameterDefinition {
        id: 5,
        protocol_number: 10,
        name: b"VCA Level",
        module: b"VCA",
    },
    ParameterDefinition {
        id: 6,
        protocol_number: 11,
        name: b"Attack",
        module: b"Envelope",
    },
    ParameterDefinition {
        id: 7,
        protocol_number: 12,
        name: b"Decay",
        module: b"Envelope",
    },
    ParameterDefinition {
        id: 8,
        protocol_number: 13,
        name: b"Sustain",
        module: b"Envelope",
    },
    ParameterDefinition {
        id: 9,
        protocol_number: 14,
        name: b"Release",
        module: b"Envelope",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    Channel,
    Parameter,
    Value,
    Controller,
    Range,
}

pub fn basic_channel_from_user_channel(channel: i32) -> Result<u8, ProtocolError> {
    if !(1..=16).contains(&channel) {
        return Err(ProtocolError::Channel);
    }
    Ok((channel - 1) as u8)
}

pub fn parameter_change(
    channel: i32,
    parameter: i32,
    value: i32,
) -> Result<[u8; 7], ProtocolError> {
    if !(0..=17).contains(&parameter) {
        return Err(ProtocolError::Parameter);
    }
    if !(0..=127).contains(&value) {
        return Err(ProtocolError::Value);
    }
    Ok([
        0xf0,
        0x41,
        0x32,
        basic_channel_from_user_channel(channel)?,
        parameter as u8,
        value as u8,
        0xf7,
    ])
}

pub fn control_change(channel: i32, controller: i32, value: i32) -> Result<[u8; 3], ProtocolError> {
    if !(0..=127).contains(&controller) {
        return Err(ProtocolError::Controller);
    }
    if !(0..=127).contains(&value) {
        return Err(ProtocolError::Value);
    }
    Ok([
        0xb0 | basic_channel_from_user_channel(channel)?,
        controller as u8,
        value as u8,
    ])
}

pub fn melody_chord_switches_1(
    range: i32,
    pulse: bool,
    saw: bool,
    chorus: bool,
) -> Result<u8, ProtocolError> {
    if !(0..=2).contains(&range) {
        return Err(ProtocolError::Range);
    }
    Ok((1 << range)
        | if pulse { 0x08 } else { 0 }
        | if saw { 0x10 } else { 0 }
        | if chorus { 0 } else { 0x20 })
}

pub const fn melody_chord_dynamics(vcf: bool, vca: bool) -> u8 {
    (if vcf { 0x40 } else { 0 }) | (if vca { 0x20 } else { 0 })
}

pub const fn melody_chord_switches_2(
    pwm_manual: bool,
    env_negative: bool,
    vca_gate: bool,
    hpf: bool,
    noise: bool,
) -> u8 {
    (if pwm_manual { 0x01 } else { 0 })
        | (if env_negative { 0x02 } else { 0 })
        | (if vca_gate { 0x04 } else { 0 })
        | (if hpf { 0 } else { 0x10 })
        | (if noise { 0x20 } else { 0 })
}

pub const fn bass_switches_1(saw: bool) -> u8 {
    if saw { 0x10 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_verified_messages_and_switches() {
        assert_eq!(basic_channel_from_user_channel(1), Ok(0));
        assert_eq!(basic_channel_from_user_channel(16), Ok(15));
        assert_eq!(
            parameter_change(1, 5, 96),
            Ok([0xf0, 0x41, 0x32, 0, 5, 0x60, 0xf7])
        );
        assert_eq!(control_change(3, 121, 127), Ok([0xb2, 0x79, 0x7f]));
        assert_eq!(melody_chord_switches_1(0, false, false, false), Ok(0x21));
        assert_eq!(melody_chord_switches_1(1, true, false, false), Ok(0x2a));
        assert_eq!(melody_chord_switches_1(1, false, true, false), Ok(0x32));
        assert_eq!(melody_chord_switches_1(1, true, true, false), Ok(0x3a));
        assert_eq!(melody_chord_switches_1(2, true, true, true), Ok(0x1c));
        assert_eq!(melody_chord_dynamics(true, true), 0x60);
        assert_eq!(
            melody_chord_switches_2(false, false, false, false, false),
            0x10
        );
        assert_eq!(melody_chord_switches_2(true, true, true, true, false), 0x07);
        assert_eq!(melody_chord_switches_2(true, true, true, false, true), 0x37);
        assert_eq!(bass_switches_1(false), 0);
        assert_eq!(bass_switches_1(true), 0x10);
    }

    #[test]
    fn rejects_out_of_range_fields_and_preserves_definitions() {
        assert_eq!(parameter_change(1, 5, 128), Err(ProtocolError::Value));
        assert_eq!(
            melody_chord_switches_1(3, true, false, false),
            Err(ProtocolError::Range)
        );
        assert_eq!(
            MELODY_PARAMETERS.map(|p| p.protocol_number),
            [0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
        );
        assert_eq!(
            BASS_PARAMETERS.map(|p| p.protocol_number),
            [3, 5, 6, 7, 9, 10, 11, 12, 13, 14]
        );
    }
}
