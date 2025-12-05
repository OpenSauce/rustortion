use anyhow::{Context, Result};
use jack::Client;
use log::{debug, error};

use crate::audio::engine::Engine;
use crate::audio::ports::Ports;

pub struct NotificationHandler;
pub struct ProcessHandler {
    ports: Ports,
    audio_engine: Engine,
    buffer: Vec<f32>,
    phase: f32,
}

impl jack::NotificationHandler for NotificationHandler {
    fn sample_rate(&mut self, _: &Client, sample_rate: jack::Frames) -> jack::Control {
        debug!(">> JACK sample_rate changed to {sample_rate}");

        jack::Control::Continue
    }
}

impl ProcessHandler {
    pub fn new(client: &Client, audio_engine: Engine) -> Result<Self> {
        let ports = Ports::new(client).context("failed to create audio ports")?;
        let buffer_size = client.buffer_size() as usize;

        Ok(Self {
            ports,
            audio_engine,
            buffer: vec![0.0; buffer_size],
            phase: 0.0,
        })
    }
}

impl jack::ProcessHandler for ProcessHandler {
    fn process(&mut self, _client: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
        let input = self.ports.get_input(ps);

        if let Err(e) = self.audio_engine.process(input, self.buffer.as_mut_slice()) {
            error!("Audio processing error: {}", e);
            self.ports.silence_output(ps);
            return jack::Control::Continue;
        };

        let frequency = 440.0; // A4
        let sample_rate = 48000.0;
        let phase_increment = frequency * 2.0 * std::f32::consts::PI / sample_rate;

        for v in self.buffer.iter_mut() {
            *v = self.phase.sin() * 0.5; // 0.5 amplitude to avoid clipping
            self.phase += phase_increment;
            if self.phase > 2.0 * std::f32::consts::PI {
                self.phase -= 2.0 * std::f32::consts::PI;
            }
        }

        self.ports.write_output(ps, &self.buffer);
        jack::Control::Continue
    }

    fn buffer_size(&mut self, _client: &jack::Client, frames: jack::Frames) -> jack::Control {
        debug!(">> JACK buffer_size changed to {frames} frames");

        let new_size = frames as usize;
        self.buffer.resize(new_size, 0.0);

        if let Err(e) = self.audio_engine.update_buffer_size(new_size) {
            error!("Failed to update buffer size: {}", e);
            return jack::Control::Continue;
        }

        jack::Control::Continue
    }
}
