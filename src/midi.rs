use coremidi::{Client, Destination, Destinations, PacketBuffer};

struct Output {
    destination: Option<Destination>,
    unique_id: i32,
    name: String,
}

pub struct MidiBackend {
    _client: Option<Client>,
    port: Option<coremidi::OutputPort>,
    outputs: Vec<Output>,
}

impl MidiBackend {
    pub fn new() -> Self {
        let mut outputs = vec![Output {
            destination: None,
            unique_id: 0,
            name: "None".into(),
        }];
        for (index, destination) in Destinations.into_iter().enumerate() {
            let unique_id = destination.unique_id().unwrap_or(0) as i32;
            let name = destination
                .display_name()
                .unwrap_or_else(|| format!("MIDI Output {}", index + 1));
            outputs.push(Output {
                destination: Some(destination),
                unique_id,
                name,
            });
        }
        let client = Client::new("MKS-7 CLAP Controller").ok();
        let port = client
            .as_ref()
            .and_then(|client| client.output_port("MKS-7 CLAP Controller SysEx").ok());
        Self {
            _client: client,
            port,
            outputs,
        }
    }

    pub fn count(&self) -> usize {
        self.outputs.len().max(1)
    }
    pub fn name(&self, index: usize) -> &str {
        self.outputs
            .get(index)
            .map(|o| o.name.as_str())
            .unwrap_or("None")
    }
    pub fn unique_ids(&self) -> Vec<i32> {
        self.outputs.iter().map(|o| o.unique_id).collect()
    }
    pub fn unique_id(&self, index: usize) -> i32 {
        self.outputs.get(index).map(|o| o.unique_id).unwrap_or(0)
    }

    pub fn send(&self, index: usize, bytes: &[u8]) -> bool {
        let Some(port) = &self.port else {
            return false;
        };
        let Some(destination) = self.outputs.get(index).and_then(|o| o.destination.as_ref()) else {
            return false;
        };
        #[cfg(debug_assertions)]
        eprintln!(
            "MKS-7 CLAP Controller MIDI: {}",
            bytes
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        port.send(destination, &PacketBuffer::new(0, bytes)).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::MidiBackend;
    use coremidi::Client;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn enumerates_unique_id_and_sends_to_coremidi_destination() {
        let (sender, receiver) = mpsc::channel();
        let client = Client::new("MKS-7 CLAP Controller Test").unwrap();
        let destination = client
            .virtual_destination("MKS-7 CLAP Controller Test Destination", move |packets| {
                let bytes = packets
                    .iter()
                    .flat_map(|packet| packet.data().iter().copied())
                    .collect::<Vec<_>>();
                let _ = sender.send(bytes);
            })
            .unwrap();
        let expected_id = destination.unique_id().unwrap_or(0) as i32;

        let backend = MidiBackend::new();
        let index = (0..backend.count())
            .find(|index| backend.name(*index) == "MKS-7 CLAP Controller Test Destination")
            .expect("virtual CoreMIDI destination was not enumerated");
        assert_eq!(backend.unique_id(index), expected_id);
        let message = [0xf0, 0x41, 0x32, 0, 5, 96, 0xf7];
        assert!(backend.send(index, &message));
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            message
        );
    }
}
