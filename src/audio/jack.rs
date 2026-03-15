use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use jack::Client;
use log::{error, warn};

use crate::audio::engine::Engine;
use crate::audio::ports::Ports;

pub struct NotificationHandler {
    xrun_count: Arc<AtomicU64>,
}

pub struct ProcessHandler {
    ports: Ports,
    audio_engine: Engine,
    buffer: Vec<f32>,
    metronome_buffer: Vec<f32>,
    max_buffer_capacity: usize,
}

impl NotificationHandler {
    pub const fn new(xrun_count: Arc<AtomicU64>) -> Self {
        Self { xrun_count }
    }
}

impl jack::NotificationHandler for NotificationHandler {
    fn sample_rate(&mut self, _: &Client, sample_rate: jack::Frames) -> jack::Control {
        warn!("JACK sample_rate changed to {sample_rate}");

        jack::Control::Continue
    }

    fn xrun(&mut self, _: &Client) -> jack::Control {
        self.xrun_count.fetch_add(1, Ordering::Relaxed);
        jack::Control::Continue
    }
}

impl ProcessHandler {
    const MAX_BUFFER_FRAMES: usize = 8192;

    pub fn new(client: &Client, audio_engine: Engine) -> Result<Self> {
        let ports = Ports::new(client).context("failed to create audio ports")?;
        let buffer_size = client.buffer_size() as usize;
        let max_capacity = Self::MAX_BUFFER_FRAMES.max(buffer_size);

        let mut buffer = Vec::with_capacity(max_capacity);
        buffer.resize(buffer_size, 0.0);
        let mut metronome_buffer = Vec::with_capacity(max_capacity);
        metronome_buffer.resize(buffer_size, 0.0);

        Ok(Self {
            ports,
            audio_engine,
            buffer,
            metronome_buffer,
            max_buffer_capacity: max_capacity,
        })
    }
}

impl jack::ProcessHandler for ProcessHandler {
    fn process(&mut self, _client: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
        let input = self.ports.get_input(ps);

        if let Err(e) = self.audio_engine.process(input, self.buffer.as_mut_slice()) {
            error!("Audio processing error: {e}");
            self.ports.silence_output(ps);
            return jack::Control::Continue;
        }
        if self
            .audio_engine
            .process_metronome(self.metronome_buffer.as_mut_slice())
        {
            self.ports
                .write_metronome_output(ps, &self.metronome_buffer);
        }

        self.ports.write_output(ps, &self.buffer);
        jack::Control::Continue
    }

    fn buffer_size(&mut self, _client: &jack::Client, frames: jack::Frames) -> jack::Control {
        let new_size = frames as usize;

        if new_size > self.max_buffer_capacity {
            if let Err(e) = self
                .buffer
                .try_reserve(new_size.saturating_sub(self.buffer.capacity()))
            {
                error!("Failed to grow audio buffer for JACK buffer_size {new_size}: {e}");
                return jack::Control::Quit;
            }

            if let Err(e) = self
                .metronome_buffer
                .try_reserve(new_size.saturating_sub(self.metronome_buffer.capacity()))
            {
                error!("Failed to grow metronome buffer for JACK buffer_size {new_size}: {e}");
                return jack::Control::Quit;
            }

            self.max_buffer_capacity = new_size;
        }

        warn!("JACK buffer_size changed to {frames} frames");
        self.buffer.resize(new_size, 0.0);
        self.metronome_buffer.resize(new_size, 0.0);

        if let Err(e) = self.audio_engine.update_buffer_size(new_size) {
            error!("Failed to update buffer size: {e}");
        }

        jack::Control::Continue
    }
}
