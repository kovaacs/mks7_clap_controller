mod midi;
mod model;
mod protocol;

use clack_extensions::note_ports::{
    NoteDialect, NoteDialects, NotePortInfo, NotePortInfoWriter, PluginNotePorts,
    PluginNotePortsImpl,
};
use clack_extensions::params::{
    HostParams, ParamDisplayWriter, ParamInfo, ParamInfoFlags, ParamInfoWriter, ParamRescanFlags,
    PluginAudioProcessorParams, PluginMainThreadParams, PluginParams,
};
use clack_extensions::remote_controls::{
    PluginRemoteControls, PluginRemoteControlsImpl, RemoteControlsPage, RemoteControlsPageWriter,
};
use clack_extensions::state::{PluginState, PluginStateImpl};
use clack_plugin::entry::prelude::*;
use clack_plugin::events::spaces::CoreEventSpace;
use clack_plugin::prelude::*;
use clack_plugin::stream::{InputStream, OutputStream};
use midi::MidiBackend;
use model::*;
use std::ffi::CStr;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct Mks7Plugin;

impl Plugin for Mks7Plugin {
    type AudioProcessor<'a> = Mks7AudioProcessor<'a>;
    type Shared<'a> = Mks7Shared;
    type MainThread<'a> = Mks7MainThread<'a>;

    fn declare_extensions(builder: &mut PluginExtensions<Self>, _shared: Option<&Mks7Shared>) {
        builder
            .register::<PluginParams>()
            .register::<PluginState>()
            .register::<PluginNotePorts>()
            .register::<PluginRemoteControls>();
    }
}

pub struct Mks7Shared {
    state: Arc<State>,
    midi: Arc<MidiBackend>,
    running: Arc<AtomicBool>,
    sender: Mutex<Option<JoinHandle<()>>>,
}

impl Mks7Shared {
    fn new(part: Part) -> Self {
        let state = Arc::new(State::new(part));
        let midi = Arc::new(MidiBackend::new());
        let running = Arc::new(AtomicBool::new(true));
        let thread_state = Arc::clone(&state);
        let thread_midi = Arc::clone(&midi);
        let thread_running = Arc::clone(&running);
        let sender = thread::spawn(move || {
            let mut next_parameter = 0;
            while thread_running.load(Ordering::Relaxed) {
                let output = thread_state.output_index();
                if output > 0
                    && output < thread_midi.count()
                    && thread_state.has_dirty()
                    && let Some(message) = thread_state.take_next_message(&mut next_parameter)
                {
                    if thread_midi.send(output, &message.bytes) {
                        thread::sleep(Duration::from_millis(25));
                    } else {
                        thread_state.retry(message.kind);
                        thread::sleep(Duration::from_millis(5));
                    }
                    continue;
                }
                thread::sleep(Duration::from_millis(5));
            }
        });
        Self {
            state,
            midi,
            running,
            sender: Mutex::new(Some(sender)),
        }
    }
}

impl Drop for Mks7Shared {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(sender) = self.sender.lock().ok().and_then(|mut sender| sender.take()) {
            let _ = sender.join();
        }
    }
}

impl PluginShared<'_> for Mks7Shared {}

pub struct Mks7MainThread<'a> {
    shared: &'a Mks7Shared,
    host: HostMainThreadHandle<'a>,
    host_params: Option<HostParams>,
}

impl<'a> PluginMainThread<'a, Mks7Shared> for Mks7MainThread<'a> {}

pub struct Mks7AudioProcessor<'a> {
    shared: &'a Mks7Shared,
}

impl<'a> PluginAudioProcessor<'a, Mks7Shared, Mks7MainThread<'a>> for Mks7AudioProcessor<'a> {
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &Mks7MainThread,
        shared: &'a Mks7Shared,
        _audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        if shared.state.has_authoritative_state() {
            shared.state.mark_all_dirty();
        }
        Ok(Self { shared })
    }

    fn process(
        &mut self,
        _process: Process,
        _audio: Audio,
        events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        for event in events.input {
            match event.as_core_event() {
                Some(CoreEventSpace::ParamValue(value)) => {
                    if let Some(id) = value.param_id() {
                        self.shared.state.apply_value(
                            id.get(),
                            value.value(),
                            self.shared.midi.count(),
                        );
                    }
                }
                Some(CoreEventSpace::ParamMod(value)) => {
                    if let Some(id) = value.param_id() {
                        self.shared.state.apply_modulation(id.get(), value.amount());
                    }
                }
                Some(CoreEventSpace::NoteOn(_))
                | Some(CoreEventSpace::NoteOff(_))
                | Some(CoreEventSpace::NoteChoke(_))
                | Some(CoreEventSpace::NoteExpression(_))
                | Some(CoreEventSpace::Midi(_))
                | Some(CoreEventSpace::MidiSysEx(_))
                | Some(CoreEventSpace::Midi2(_)) => {
                    let _ = events.output.try_push(event);
                }
                _ => {}
            }
        }
        Ok(ProcessStatus::Sleep)
    }

    fn reset(&mut self) {
        if self.shared.state.has_authoritative_state() {
            self.shared.state.mark_all_dirty();
        }
    }
}

fn flush_events(shared: &Mks7Shared, input: &InputEvents) {
    for event in input {
        match event.as_core_event() {
            Some(CoreEventSpace::ParamValue(value)) => {
                if let Some(id) = value.param_id() {
                    shared
                        .state
                        .apply_value(id.get(), value.value(), shared.midi.count());
                }
            }
            Some(CoreEventSpace::ParamMod(value)) => {
                if let Some(id) = value.param_id() {
                    shared.state.apply_modulation(id.get(), value.amount());
                }
            }
            _ => {}
        }
    }
}

impl PluginAudioProcessorParams for Mks7AudioProcessor<'_> {
    fn flush(&mut self, input: &InputEvents, _output: &mut OutputEvents) {
        flush_events(self.shared, input);
    }
}

impl PluginMainThreadParams for Mks7MainThread<'_> {
    fn count(&self) -> u32 {
        self.shared.state.part.parameter_count() as u32
    }

    fn get_info(&self, index: u32, writer: &mut ParamInfoWriter) {
        let part = self.shared.state.part;
        let base_count = part.parameters().len();
        let index = index as usize;
        if let Some(definition) = part.parameters().get(index) {
            writer.set(&ParamInfo {
                id: ClapId::new(definition.id),
                flags: ParamInfoFlags::IS_STEPPED
                    | ParamInfoFlags::IS_AUTOMATABLE
                    | ParamInfoFlags::IS_MODULATABLE
                    | ParamInfoFlags::REQUIRES_PROCESS,
                cookie: Default::default(),
                name: definition.name,
                module: definition.module,
                min_value: 0.0,
                max_value: 127.0,
                default_value: 0.0,
            });
            return;
        }
        if index < base_count + part.switch_control_count() {
            let switch_index = index - base_count;
            let (id, name, module, max, default): (u32, &[u8], &[u8], f64, f64) = if part.is_bass()
            {
                (
                    BASS_WAVEFORM_PARAMETER_ID,
                    b"Waveform".as_slice(),
                    b"DCO".as_slice(),
                    1.0,
                    0.0,
                )
            } else {
                match switch_index {
                    0 => (RANGE_PARAMETER_ID, b"Range", b"DCO", 2.0, 1.0),
                    1 => (PULSE_PARAMETER_ID, b"Pulse", b"DCO", 1.0, 1.0),
                    2 => (SAW_PARAMETER_ID, b"Saw", b"DCO", 1.0, 0.0),
                    3 => (CHORUS_PARAMETER_ID, b"Chorus", b"DCO / Chorus", 1.0, 0.0),
                    4 => (
                        VCF_DYNAMICS_PARAMETER_ID,
                        b"VCF Dynamics",
                        b"Dynamics",
                        1.0,
                        0.0,
                    ),
                    5 => (
                        VCA_DYNAMICS_PARAMETER_ID,
                        b"VCA Dynamics",
                        b"Dynamics",
                        1.0,
                        0.0,
                    ),
                    6 => (PWM_SOURCE_PARAMETER_ID, b"PWM Source", b"DCO", 1.0, 0.0),
                    7 => (VCA_MODE_PARAMETER_ID, b"VCA Mode", b"VCA", 1.0, 0.0),
                    8 => (
                        ENV_POLARITY_PARAMETER_ID,
                        b"VCF ENV Polarity",
                        b"VCF",
                        1.0,
                        0.0,
                    ),
                    9 => (HPF_PARAMETER_ID, b"HPF", b"VCF", 1.0, 0.0),
                    _ => (NOISE_PARAMETER_ID, b"Noise", b"DCO", 1.0, 0.0),
                }
            };
            writer.set(&ParamInfo {
                id: ClapId::new(id),
                flags: ParamInfoFlags::IS_STEPPED
                    | ParamInfoFlags::IS_ENUM
                    | ParamInfoFlags::IS_AUTOMATABLE
                    | ParamInfoFlags::REQUIRES_PROCESS,
                cookie: Default::default(),
                name,
                module,
                min_value: 0.0,
                max_value: max,
                default_value: default,
            });
            return;
        }
        let offset = base_count + part.switch_control_count();
        if index == offset {
            let name = match part {
                Part::Melody => b"Melody Channel".as_slice(),
                Part::Chord => b"Chord Channel",
                Part::Bass => b"Bass Channel",
            };
            writer.set(&ParamInfo {
                id: ClapId::new(CHANNEL_PARAMETER_ID),
                flags: ParamInfoFlags::IS_STEPPED
                    | ParamInfoFlags::IS_ENUM
                    | ParamInfoFlags::REQUIRES_PROCESS,
                cookie: Default::default(),
                name,
                module: b"MIDI",
                min_value: 1.0,
                max_value: 16.0,
                default_value: part.default_channel() as f64,
            });
        } else if index == offset + 1 {
            writer.set(&ParamInfo {
                id: ClapId::new(MIDI_OUTPUT_PARAMETER_ID),
                flags: ParamInfoFlags::IS_STEPPED | ParamInfoFlags::IS_ENUM,
                cookie: Default::default(),
                name: b"MIDI Output",
                module: b"MIDI",
                min_value: 0.0,
                max_value: self.shared.midi.count().saturating_sub(1) as f64,
                default_value: 0.0,
            });
        } else if index == offset + 2 && part.supports_whole_mode() {
            writer.set(&ParamInfo {
                id: ClapId::new(WHOLE_MODE_PARAMETER_ID),
                flags: ParamInfoFlags::IS_STEPPED
                    | ParamInfoFlags::IS_ENUM
                    | ParamInfoFlags::IS_AUTOMATABLE
                    | ParamInfoFlags::REQUIRES_PROCESS,
                cookie: Default::default(),
                name: b"Whole Mode",
                module: b"Voice",
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.0,
            });
        }
    }

    fn get_value(&self, id: ClapId) -> Option<f64> {
        self.shared.state.get_value(id.get())
    }

    fn value_to_text(
        &self,
        id: ClapId,
        value: f64,
        writer: &mut ParamDisplayWriter,
    ) -> std::fmt::Result {
        let id = id.get();
        let part = self.shared.state.part;
        let text = if id == MIDI_OUTPUT_PARAMETER_ID {
            let index = (value.round() as isize)
                .clamp(0, self.shared.midi.count().saturating_sub(1) as isize)
                as usize;
            self.shared.midi.name(index)
        } else if id == WHOLE_MODE_PARAMETER_ID && part.supports_whole_mode() {
            if value >= 0.5 { "6 Voices" } else { "4 Voices" }
        } else if id == RANGE_PARAMETER_ID && part.supports_oscillator_switches() {
            ["16'", "8'", "4'"][(value.round() as i32).clamp(0, 2) as usize]
        } else if part.supports_oscillator_switches()
            && matches!(
                id,
                PULSE_PARAMETER_ID
                    | SAW_PARAMETER_ID
                    | CHORUS_PARAMETER_ID
                    | VCF_DYNAMICS_PARAMETER_ID
                    | VCA_DYNAMICS_PARAMETER_ID
                    | HPF_PARAMETER_ID
            )
            || id == NOISE_PARAMETER_ID && part.is_melody()
        {
            if value >= 0.5 { "On" } else { "Off" }
        } else if id == PWM_SOURCE_PARAMETER_ID && part.supports_oscillator_switches() {
            if value >= 0.5 { "Manual" } else { "LFO" }
        } else if id == VCA_MODE_PARAMETER_ID && part.supports_oscillator_switches() {
            if value >= 0.5 { "Gate" } else { "ENV" }
        } else if id == ENV_POLARITY_PARAMETER_ID && part.supports_oscillator_switches() {
            if value >= 0.5 { "Negative" } else { "Positive" }
        } else if id == BASS_WAVEFORM_PARAMETER_ID && part.is_bass() {
            if value >= 0.5 { "Saw" } else { "Pulse" }
        } else if (id as usize) < part.parameters().len() || id == CHANNEL_PARAMETER_ID {
            return write!(writer, "{}", value.round() as i64);
        } else {
            return Err(std::fmt::Error);
        };
        writer.write_str(text)
    }

    fn text_to_value(&self, id: ClapId, text: &CStr) -> Option<f64> {
        let id = id.get();
        self.shared.state.get_value(id)?;
        let text = text.to_str().ok()?;
        let part = self.shared.state.part;
        let parsed = text.parse::<f64>().ok().or_else(|| {
            if id == MIDI_OUTPUT_PARAMETER_ID {
                (0..self.shared.midi.count())
                    .find(|index| self.shared.midi.name(*index) == text)
                    .map(|index| index as f64)
            } else if id == WHOLE_MODE_PARAMETER_ID && part.supports_whole_mode() {
                match text {
                    "4 Voices" => Some(0.0),
                    "6 Voices" => Some(1.0),
                    _ => None,
                }
            } else if id == RANGE_PARAMETER_ID && part.supports_oscillator_switches() {
                match text {
                    "16'" => Some(0.0),
                    "8'" => Some(1.0),
                    "4'" => Some(2.0),
                    _ => None,
                }
            } else if part.supports_oscillator_switches()
                && matches!(
                    id,
                    PULSE_PARAMETER_ID
                        | SAW_PARAMETER_ID
                        | CHORUS_PARAMETER_ID
                        | VCF_DYNAMICS_PARAMETER_ID
                        | VCA_DYNAMICS_PARAMETER_ID
                        | HPF_PARAMETER_ID
                )
                || id == NOISE_PARAMETER_ID && part.is_melody()
            {
                match text {
                    "Off" => Some(0.0),
                    "On" => Some(1.0),
                    _ => None,
                }
            } else if id == PWM_SOURCE_PARAMETER_ID && part.supports_oscillator_switches() {
                match text {
                    "LFO" => Some(0.0),
                    "Manual" => Some(1.0),
                    _ => None,
                }
            } else if id == VCA_MODE_PARAMETER_ID && part.supports_oscillator_switches() {
                match text {
                    "ENV" => Some(0.0),
                    "Gate" => Some(1.0),
                    _ => None,
                }
            } else if id == ENV_POLARITY_PARAMETER_ID && part.supports_oscillator_switches() {
                match text {
                    "Positive" => Some(0.0),
                    "Negative" => Some(1.0),
                    _ => None,
                }
            } else if id == BASS_WAVEFORM_PARAMETER_ID && part.is_bass() {
                match text {
                    "Pulse" => Some(0.0),
                    "Saw" => Some(1.0),
                    _ => None,
                }
            } else {
                None
            }
        })?;
        if !parsed.is_finite() {
            return None;
        }
        let min = if id == CHANNEL_PARAMETER_ID { 1.0 } else { 0.0 };
        let max = match id {
            CHANNEL_PARAMETER_ID => 16.0,
            MIDI_OUTPUT_PARAMETER_ID => self.shared.midi.count().saturating_sub(1) as f64,
            RANGE_PARAMETER_ID => 2.0,
            WHOLE_MODE_PARAMETER_ID
            | PULSE_PARAMETER_ID
            | SAW_PARAMETER_ID
            | CHORUS_PARAMETER_ID
            | VCF_DYNAMICS_PARAMETER_ID
            | VCA_DYNAMICS_PARAMETER_ID
            | PWM_SOURCE_PARAMETER_ID
            | VCA_MODE_PARAMETER_ID
            | ENV_POLARITY_PARAMETER_ID
            | HPF_PARAMETER_ID
            | NOISE_PARAMETER_ID
            | BASS_WAVEFORM_PARAMETER_ID => 1.0,
            _ => 127.0,
        };
        (parsed >= min && parsed <= max).then(|| parsed.round())
    }

    fn flush(&self, input: &InputEvents, _output: &mut OutputEvents) {
        flush_events(self.shared, input);
    }
}

impl PluginStateImpl for Mks7MainThread<'_> {
    fn save(&self, output: &mut OutputStream) -> Result<(), PluginError> {
        let id = self.shared.midi.unique_id(self.shared.state.output_index());
        output.write_all(&self.shared.state.save(id))?;
        Ok(())
    }

    fn load(&self, input: &mut InputStream) -> Result<(), PluginError> {
        let mut header = [0; 5];
        input.read_exact(&mut header)?;
        let size = self
            .shared
            .state
            .serialized_size(header[4])
            .ok_or(PluginError::Message(
                "Invalid MKS-7 Controller state version",
            ))?;
        let mut data = vec![0; size];
        data[..5].copy_from_slice(&header);
        input.read_exact(&mut data[5..])?;
        self.shared
            .state
            .load(&data, &self.shared.midi.unique_ids())
            .map_err(|_| PluginError::Message("Invalid MKS-7 Controller state"))?;
        if let Some(params) = &self.host_params {
            params.rescan(&self.host, ParamRescanFlags::VALUES);
        }
        Ok(())
    }
}

impl PluginNotePortsImpl for Mks7MainThread<'_> {
    fn count(&self, _is_input: bool) -> u32 {
        1
    }
    fn get(&self, index: u32, _is_input: bool, writer: &mut NotePortInfoWriter) {
        if index == 0 {
            writer.set(&NotePortInfo {
                id: ClapId::new(0),
                name: b"MKS-7 Controller MIDI",
                supported_dialects: NoteDialects::CLAP
                    | NoteDialects::MIDI
                    | NoteDialects::MIDI_MPE
                    | NoteDialects::MIDI2,
                preferred_dialect: Some(NoteDialect::Clap),
            });
        }
    }
}

fn ids(values: &[u32]) -> [Option<ClapId>; 8] {
    let mut result = [None; 8];
    for (slot, id) in result.iter_mut().zip(values) {
        *slot = Some(ClapId::new(*id));
    }
    result
}

impl PluginRemoteControlsImpl for Mks7MainThread<'_> {
    fn count(&self) -> u32 {
        if self.shared.state.part.is_bass() {
            3
        } else {
            6
        }
    }
    fn get(&self, index: u32, writer: &mut RemoteControlsPageWriter) {
        let part = self.shared.state.part;
        if index >= PluginRemoteControlsImpl::count(self) {
            return;
        }
        let (name, param_ids) = if part.is_bass() {
            match index {
                0 => (
                    b"DCO and VCF".as_slice(),
                    ids(&[0, BASS_WAVEFORM_PARAMETER_ID, 1, 2, 3, 4]),
                ),
                1 => (b"VCA and ENV".as_slice(), ids(&[5, 6, 7, 8, 9])),
                _ => (
                    b"MIDI".as_slice(),
                    ids(&[MIDI_OUTPUT_PARAMETER_ID, CHANNEL_PARAMETER_ID]),
                ),
            }
        } else {
            match index {
                0 => (b"LFO and DCO".as_slice(), ids(&[0, 1, 2, 3, 14])),
                1 => (b"VCF".as_slice(), ids(&[4, 5, 6, 7, 8])),
                2 => (b"VCA and ENV".as_slice(), ids(&[9, 10, 11, 12, 13])),
                3 => {
                    let mut page = ids(&[
                        RANGE_PARAMETER_ID,
                        PULSE_PARAMETER_ID,
                        SAW_PARAMETER_ID,
                        CHORUS_PARAMETER_ID,
                        PWM_SOURCE_PARAMETER_ID,
                    ]);
                    if part.is_melody() {
                        page[5] = Some(ClapId::new(NOISE_PARAMETER_ID));
                    }
                    (b"DCO Switches".as_slice(), page)
                }
                4 => (
                    b"Modes and Dynamics".as_slice(),
                    ids(&[
                        VCF_DYNAMICS_PARAMETER_ID,
                        VCA_DYNAMICS_PARAMETER_ID,
                        VCA_MODE_PARAMETER_ID,
                        ENV_POLARITY_PARAMETER_ID,
                        HPF_PARAMETER_ID,
                    ]),
                ),
                _ => {
                    let mut page = ids(&[MIDI_OUTPUT_PARAMETER_ID, CHANNEL_PARAMETER_ID]);
                    if part.supports_whole_mode() {
                        page[2] = Some(ClapId::new(WHOLE_MODE_PARAMETER_ID));
                    }
                    (b"MIDI".as_slice(), page)
                }
            }
        };
        writer.set(&RemoteControlsPage {
            section_name: part.name(),
            page_id: ClapId::new(200 + index),
            page_name: name,
            param_ids,
            is_for_preset: false,
        });
    }
}

struct Mks7Factory {
    descriptors: [PluginDescriptor; 3],
}

impl Mks7Factory {
    fn new() -> Self {
        use clack_plugin::plugin::features::{NOTE_EFFECT, UTILITY};
        let descriptor = |part: &str, name: &str| {
            PluginDescriptor::new(&format!("com.marcellkovacs.mks7-controller-{part}"), name)
                .with_vendor("Marcell Kovacs")
                .with_version(env!("CARGO_PKG_VERSION"))
                .with_description(&format!("Roland {name} controller and SysEx note effect"))
                .with_features([NOTE_EFFECT, UTILITY])
        };
        Self {
            descriptors: [
                descriptor("melody", "MKS-7 Melody Controller"),
                descriptor("chord", "MKS-7 Chord Controller"),
                descriptor("bass", "MKS-7 Bass Controller"),
            ],
        }
    }
}

impl PluginFactoryImpl for Mks7Factory {
    fn plugin_count(&self) -> u32 {
        3
    }
    fn plugin_descriptor(&self, index: u32) -> Option<&PluginDescriptor> {
        self.descriptors.get(index as usize)
    }
    fn create_plugin<'a>(
        &'a self,
        host_info: HostInfo<'a>,
        id: &CStr,
    ) -> Option<PluginInstance<'a>> {
        let index = self
            .descriptors
            .iter()
            .position(|descriptor| descriptor.id() == Some(id))?;
        let descriptor = &self.descriptors[index];
        let part = [Part::Melody, Part::Chord, Part::Bass][index];
        Some(PluginInstance::new::<Mks7Plugin>(
            host_info,
            descriptor,
            move |_host| Ok(Mks7Shared::new(part)),
            |host, shared| {
                Ok(Mks7MainThread {
                    shared,
                    host_params: host.get_extension::<HostParams>(),
                    host,
                })
            },
        ))
    }
}

pub struct Mks7Entry {
    factory: PluginFactoryWrapper<Mks7Factory>,
}

impl Entry for Mks7Entry {
    fn new(_bundle_path: Option<&CStr>) -> Result<Self, EntryLoadError> {
        Ok(Self {
            factory: PluginFactoryWrapper::new(Mks7Factory::new()),
        })
    }
    fn declare_factories<'a>(&'a self, builder: &mut EntryFactories<'a>) {
        builder.register_factory(&self.factory);
    }
}

clack_plugin::clack_export_entry!(Mks7Entry);
