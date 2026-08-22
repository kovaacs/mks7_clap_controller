use clack_extensions::note_ports::{NoteDialects, NotePortInfoBuffer, PluginNotePorts};
use clack_extensions::params::{ParamInfoBuffer, ParamInfoFlags, PluginParams};
use clack_extensions::remote_controls::{PluginRemoteControls, RemoteControlsPageBuffer};
use clack_extensions::state::PluginState;
use clack_host::factory::plugin::PluginFactory;
use clack_host::prelude::*;
use mks7_clap_controller::Mks7Entry;

#[test]
fn descriptors_parameters_ports_and_remote_pages_are_stable() {
    let info = HostInfo::new("MKS-7 CLAP Controller test", "", "", "1.0").unwrap();
    let entry = PluginEntry::load_from_clack::<Mks7Entry>(c"").unwrap();
    let factory = entry.get_factory::<PluginFactory>().unwrap();
    assert_eq!(factory.plugin_count(), 3);

    let expected = [
        (
            b"com.marcellkovacs.mks7-clap-controller-melody".as_slice(),
            b"MKS-7 Melody Controller".as_slice(),
            28,
            1.0,
            6,
        ),
        (
            b"com.marcellkovacs.mks7-clap-controller-chord".as_slice(),
            b"MKS-7 Chord Controller".as_slice(),
            28,
            3.0,
            6,
        ),
        (
            b"com.marcellkovacs.mks7-clap-controller-bass".as_slice(),
            b"MKS-7 Bass Controller".as_slice(),
            13,
            2.0,
            3,
        ),
    ];

    for (index, (id, name, parameter_count, channel, page_count)) in
        expected.into_iter().enumerate()
    {
        let descriptor = factory.plugin_descriptor(index as u32).unwrap();
        assert_eq!(descriptor.id().unwrap().to_bytes(), id);
        assert_eq!(descriptor.name().unwrap().to_bytes(), name);
        assert_eq!(descriptor.vendor().unwrap().to_bytes(), b"Marcell Kovacs");
        assert_eq!(descriptor.version().unwrap().to_bytes(), b"0.6.1");

        let mut plugin = PluginInstance::<TestHost>::new(
            |_| TestShared,
            |_| TestMain,
            &entry,
            descriptor.id().unwrap(),
            &info,
        )
        .unwrap();
        let handle = plugin.plugin_handle();
        let params = handle.get_extension::<PluginParams>().unwrap();
        assert_eq!(params.count(&handle), parameter_count);
        assert_eq!(params.get_value(&handle, ClapId::new(100)), Some(channel));

        let base_names: &[&[u8]] = if index == 2 {
            &[
                b"DCO PWM",
                b"VCF Cutoff",
                b"VCF Resonance",
                b"VCF ENV",
                b"VCF KYBD",
                b"VCA Level",
                b"Attack",
                b"Decay",
                b"Sustain",
                b"Release",
            ]
        } else {
            &[
                b"LFO Rate",
                b"LFO Delay",
                b"DCO LFO",
                b"DCO PWM",
                b"VCF Cutoff",
                b"VCF Resonance",
                b"VCF ENV",
                b"VCF LFO",
                b"VCF KYBD",
                b"VCA Level",
                b"Attack",
                b"Decay",
                b"Sustain",
                b"Release",
                b"Sub Level",
            ]
        };
        for (parameter_index, expected_name) in base_names.iter().enumerate() {
            let mut buffer = ParamInfoBuffer::new();
            let parameter = params
                .get_info(&handle, parameter_index as u32, &mut buffer)
                .unwrap();
            assert_eq!(parameter.id, parameter_index as u32);
            assert_eq!(parameter.name, *expected_name);
            assert_eq!(
                (
                    parameter.min_value,
                    parameter.max_value,
                    parameter.default_value
                ),
                (0.0, 127.0, 0.0)
            );
            assert_eq!(
                parameter.flags,
                ParamInfoFlags::IS_STEPPED
                    | ParamInfoFlags::IS_AUTOMATABLE
                    | ParamInfoFlags::IS_MODULATABLE
                    | ParamInfoFlags::REQUIRES_PROCESS
            );
        }

        let switch_names: &[(&[u8], u32, f64)] = if index == 2 {
            &[(b"Waveform", 114, 0.0)]
        } else if index == 0 {
            &[
                (b"Range", 103, 1.0),
                (b"Pulse", 104, 1.0),
                (b"Saw", 105, 0.0),
                (b"Chorus", 106, 0.0),
                (b"VCF Dynamics", 107, 0.0),
                (b"VCA Dynamics", 108, 0.0),
                (b"PWM Source", 109, 0.0),
                (b"VCA Mode", 110, 0.0),
                (b"VCF ENV Polarity", 111, 0.0),
                (b"HPF", 112, 0.0),
                (b"Noise", 113, 0.0),
            ]
        } else {
            &[
                (b"Range", 103, 1.0),
                (b"Pulse", 104, 1.0),
                (b"Saw", 105, 0.0),
                (b"Chorus", 106, 0.0),
                (b"VCF Dynamics", 107, 0.0),
                (b"VCA Dynamics", 108, 0.0),
                (b"PWM Source", 109, 0.0),
                (b"VCA Mode", 110, 0.0),
                (b"VCF ENV Polarity", 111, 0.0),
                (b"HPF", 112, 0.0),
            ]
        };
        for (offset, (expected_name, expected_id, expected_default)) in
            switch_names.iter().enumerate()
        {
            let mut buffer = ParamInfoBuffer::new();
            let parameter = params
                .get_info(&handle, (base_names.len() + offset) as u32, &mut buffer)
                .unwrap();
            assert_eq!(
                (parameter.name, parameter.id, parameter.default_value),
                (*expected_name, ClapId::new(*expected_id), *expected_default)
            );
            assert_eq!(
                parameter.flags,
                ParamInfoFlags::IS_STEPPED
                    | ParamInfoFlags::IS_ENUM
                    | ParamInfoFlags::IS_AUTOMATABLE
                    | ParamInfoFlags::REQUIRES_PROCESS
            );
        }

        let mut text = [0; 32];
        if index == 0 {
            assert_eq!(
                params
                    .value_to_text(&handle, ClapId::new(103), 2.0, &mut text)
                    .unwrap(),
                b"4'"
            );
            assert_eq!(
                params
                    .value_to_text(&handle, ClapId::new(109), 1.0, &mut text)
                    .unwrap(),
                b"Manual"
            );
            assert_eq!(
                params.text_to_value(&handle, ClapId::new(109), c"Manual"),
                Some(1.0)
            );
        } else if index == 1 {
            assert_eq!(
                params
                    .value_to_text(&handle, ClapId::new(102), 1.0, &mut text)
                    .unwrap(),
                b"6 Voices"
            );
            assert_eq!(
                params.text_to_value(&handle, ClapId::new(102), c"6 Voices"),
                Some(1.0)
            );
        } else {
            assert_eq!(
                params
                    .value_to_text(&handle, ClapId::new(114), 0.0, &mut text)
                    .unwrap(),
                b"Pulse"
            );
            assert_eq!(
                params.text_to_value(&handle, ClapId::new(114), c"Pulse"),
                Some(0.0)
            );
        }
        assert_eq!(
            params.text_to_value(&handle, ClapId::new(101), c"None"),
            Some(0.0)
        );

        let note_ports = handle.get_extension::<PluginNotePorts>().unwrap();
        assert_eq!(note_ports.count(&handle, true), 1);
        assert_eq!(note_ports.count(&handle, false), 1);
        let mut note_buffer = NotePortInfoBuffer::new();
        let note = note_ports.get(&handle, 0, true, &mut note_buffer).unwrap();
        assert_eq!(note.id, 0);
        assert_eq!(note.name, b"MKS-7 CLAP Controller MIDI");
        assert_eq!(
            note.supported_dialects,
            NoteDialects::CLAP | NoteDialects::MIDI | NoteDialects::MIDI_MPE | NoteDialects::MIDI2
        );

        let state = handle.get_extension::<PluginState>().unwrap();
        let mut saved = Vec::new();
        state.save(&handle, &mut saved).unwrap();
        assert_eq!(&saved[..5], b"MKS7\x05");
        assert_eq!(
            saved.len(),
            if index == 2 {
                22
            } else if index == 1 {
                40
            } else {
                39
            }
        );

        let remotes = handle.get_extension::<PluginRemoteControls>().unwrap();
        assert_eq!(remotes.count(&handle), page_count);
        let expected_pages: &[(&[u8], &[Option<u32>])] = if index == 2 {
            &[
                (
                    b"DCO and VCF",
                    &[Some(0), Some(114), Some(1), Some(2), Some(3), Some(4)],
                ),
                (
                    b"VCA and ENV",
                    &[Some(5), Some(6), Some(7), Some(8), Some(9)],
                ),
                (b"MIDI", &[Some(101), Some(100)]),
            ]
        } else {
            &[
                (
                    b"LFO and DCO",
                    &[Some(0), Some(1), Some(2), Some(3), Some(14)],
                ),
                (b"VCF", &[Some(4), Some(5), Some(6), Some(7), Some(8)]),
                (
                    b"VCA and ENV",
                    &[Some(9), Some(10), Some(11), Some(12), Some(13)],
                ),
                (
                    b"DCO Switches",
                    &[
                        Some(103),
                        Some(104),
                        Some(105),
                        Some(106),
                        Some(109),
                        if index == 0 { Some(113) } else { None },
                    ],
                ),
                (
                    b"Modes and Dynamics",
                    &[Some(107), Some(108), Some(110), Some(111), Some(112)],
                ),
                (
                    b"MIDI",
                    &[
                        Some(101),
                        Some(100),
                        if index == 1 { Some(102) } else { None },
                    ],
                ),
            ]
        };
        for (page_index, (page_name, page_ids)) in expected_pages.iter().enumerate() {
            let mut page_buffer = RemoteControlsPageBuffer::new();
            let page = remotes
                .get(&handle, page_index as u32, &mut page_buffer)
                .unwrap();
            assert_eq!(page.page_id, 200 + page_index as u32);
            assert_eq!(
                page.section_name,
                &name[6..name.len() - b" Controller".len()]
            );
            assert_eq!(page.page_name, *page_name);
            let mut expected_ids = [None; 8];
            for (target, source) in expected_ids.iter_mut().zip(*page_ids) {
                *target = source.map(ClapId::new);
            }
            assert_eq!(page.param_ids, expected_ids);
            assert!(!page.is_for_preset);
        }
    }
    assert!(factory.plugin_descriptor(3).is_none());
}

struct TestMain;
struct TestShared;
struct TestAudio;
struct TestHost;

impl SharedHandler<'_> for TestShared {
    fn request_restart(&self) {}
    fn request_process(&self) {}
    fn request_callback(&self) {}
}
impl MainThreadHandler<'_> for TestMain {}
impl AudioProcessorHandler<'_> for TestAudio {}
impl HostHandlers for TestHost {
    type Shared<'a> = TestShared;
    type MainThread<'a> = TestMain;
    type AudioProcessor<'a> = TestAudio;
}
