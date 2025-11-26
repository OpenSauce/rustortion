use anyhow::Result;
use midir::{Ignore, MidiInput, MidiInputConnection};

pub struct MidiManager {
    _connection: MidiInputConnection<()>,
}

impl MidiManager {
    pub fn new() -> Result<Self> {
        let mut input = MidiInput::new("midi-listener")?;
        input.ignore(Ignore::None);

        let ports = input.ports();
        if ports.is_empty() {
            anyhow::bail!("No MIDI input ports found");
        }

        for port in ports.iter().enumerate() {
            println!("Port {}: {}", port.0, input.port_name(port.1)?);
        }

        let port = &ports[1];

        println!("Connecting to {}", input.port_name(port)?);

        let connection = input
            .connect(
                port,
                "midi-read",
                move |timestamp, message, _| println!("{}ms: {:?}", timestamp, message),
                (),
            )
            .map_err(|e| anyhow::anyhow!("Failed to connect MIDI: {}", e))?;

        println!("MIDI listening started!");
        std::thread::sleep(std::time::Duration::from_secs(999999));

        Ok(MidiManager {
            _connection: connection,
        })
    }
}
