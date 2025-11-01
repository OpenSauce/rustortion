use std::path::Path;

use crate::io::audio_ports::AudioPorts;
use crate::io::recorder::AudioBlock;
use crate::ir::cabinet::IrCabinet;
use crate::sim::chain::AmplifierChain;
use crate::sim::tuner::{Tuner, TunerInfo};
use anyhow::{Context, Result};
use crossbeam::channel::{Receiver, Sender};
use jack::{Client, Control, Frames, ProcessHandler, ProcessScope};
use log::{debug, error, info, warn};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

pub enum ProcessorMessage {
    SetAmpChain(Box<AmplifierChain>),
    SetRecording(Option<Sender<AudioBlock>>),
    SetIrCabinet(Option<String>),
    SetIrBypass(bool),
    SetIrGain(f32),
    SetTunerEnabled(bool),
}

pub struct Processor {
    /// Amplifier chain, used for processing amp simulations on the input.
    chain: Box<AmplifierChain>,
    /// IR Cabinet processor
    ir_cabinet: Option<IrCabinet>,
    /// Channel for updating the amplifier chain.
    rx_updates: Receiver<ProcessorMessage>,
    /// Optional recorder channel.
    tx_audio: Option<Sender<AudioBlock>>,
    audio_ports: AudioPorts,
    upsampler: SincFixedIn<f32>,
    downsampler: SincFixedIn<f32>,
    /// Reusable buffer for input frames.
    input_buffer: Vec<Vec<f32>>,
    /// Reusable buffer for upsampled frames.
    upsampled_buffer: Vec<Vec<f32>>,
    upsampled_frames: usize,
    /// Reusable buffer for downsampled frames.
    downsampled_buffer: Vec<Vec<f32>>,
    downsampled_frames: usize,
    oversample_factor: f64,
    tuner: Option<Tuner>,
    tx_tuner: Option<Sender<TunerInfo>>,
    sample_rate: f32,
    tuner_update_counter: usize,
}

impl Processor {
    pub fn new(
        client: &Client,
        rx_updates: Receiver<ProcessorMessage>,
        tx_audio: Option<Sender<AudioBlock>>,
        oversample_factor: f64,
        tx_tuner: Option<Sender<TunerInfo>>,
    ) -> Result<Self> {
        const CHANNELS: usize = 1;
        const MAX_BLOCK_SIZE: usize = 8192;

        let audio_ports =
            AudioPorts::new(client).context("failed to create audio ports manager")?;

        let interp_params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        let down_interp_params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };

        let mut upsampler = SincFixedIn::<f32>::new(
            oversample_factor,
            1.0,
            interp_params,
            MAX_BLOCK_SIZE,
            CHANNELS,
        )
        .context("failed to create upsampler")?;

        let mut downsampler = SincFixedIn::<f32>::new(
            1.0 / oversample_factor,
            1.0,
            down_interp_params,
            MAX_BLOCK_SIZE * oversample_factor as usize,
            CHANNELS,
        )
        .context("failed to create downsampler")?;

        let buffer_size = client.buffer_size() as usize;
        upsampler
            .set_chunk_size(buffer_size)
            .context("failed to set upsampler chunk size")?;
        downsampler
            .set_chunk_size(buffer_size * oversample_factor as usize)
            .context("failed to set downsampler chunk size")?;

        let mut input_vec = Vec::with_capacity(buffer_size);
        input_vec.resize(buffer_size, 0.0);
        let input_buffer = vec![input_vec];
        let upsampled_buffer = upsampler.output_buffer_allocate(true);
        let downsampled_buffer = downsampler.output_buffer_allocate(true);

        let ir_cabinet = match IrCabinet::new(
            Path::new("./impulse_responses"),
            client.sample_rate() as u32,
        ) {
            Ok(cab) => {
                debug!("IR Cabinet loaded successfully");
                Some(cab)
            }
            Err(e) => {
                warn!("Failed to load IR Cabinet: {}", e);
                None
            }
        };

        debug_stats(client);

        Ok(Self {
            chain: Box::new(AmplifierChain::new()),
            ir_cabinet,
            rx_updates,
            tx_audio,
            audio_ports,
            upsampler,
            downsampler,
            input_buffer,
            upsampled_buffer,
            upsampled_frames: 0,
            downsampled_buffer,
            downsampled_frames: 0,
            oversample_factor,
            tuner: None,
            tx_tuner,
            sample_rate: client.sample_rate() as f32,
            tuner_update_counter: 0,
        })
    }

    pub fn handle_messages(&mut self) {
        if let Ok(message) = self.rx_updates.try_recv() {
            match message {
                ProcessorMessage::SetAmpChain(chain) => {
                    self.chain = chain;
                    debug!("Received new amplifier chain");
                }
                ProcessorMessage::SetRecording(tx) => {
                    self.tx_audio = tx;
                    debug!("Recording channel updated");
                }
                ProcessorMessage::SetIrCabinet(ir_name) => {
                    if let Some(ref mut cab) = self.ir_cabinet
                        && let Some(name) = ir_name
                    {
                        if let Err(e) = cab.select_ir(&name) {
                            error!("Failed to set IR: {}", e);
                        } else {
                            debug!("IR Cabinet set to: {}", name);
                        }
                    }
                }
                ProcessorMessage::SetIrBypass(bypass) => {
                    if let Some(ref mut cab) = self.ir_cabinet {
                        cab.set_bypass(bypass);
                        debug!("IR Cabinet bypass: {}", bypass);
                    }
                }
                ProcessorMessage::SetIrGain(gain) => {
                    if let Some(ref mut cab) = self.ir_cabinet {
                        cab.set_gain(gain);
                        debug!("IR Cabinet gain: {}", gain);
                    }
                }
                ProcessorMessage::SetTunerEnabled(enabled) => {
                    if enabled {
                        if self.tuner.is_none() {
                            self.tuner = Some(Tuner::new(self.sample_rate));
                            debug!("Tuner enabled");
                        }
                    } else {
                        self.tuner = None;
                        debug!("Tuner disabled");
                    }
                }
            }
        }
    }

    fn process(&mut self) -> Result<()> {
        let (_, upsampled_frames) = self
            .upsampler
            .process_into_buffer(&self.input_buffer, &mut self.upsampled_buffer, None)
            .context("Upsampler failed")?;

        self.upsampled_frames = upsampled_frames;

        let chain = self.chain.as_mut();
        for s in &mut self.upsampled_buffer[0][..upsampled_frames] {
            *s = chain.process(*s);
        }

        let (_, downsampled_frames) = self
            .downsampler
            .process_into_buffer(&self.upsampled_buffer, &mut self.downsampled_buffer, None)
            .context("Downsampler failed")?;

        self.downsampled_frames = downsampled_frames;

        if let Some(ref mut cab) = self.ir_cabinet {
            cab.process_block(&mut self.downsampled_buffer[0][..downsampled_frames]);
        }

        Ok(())
    }

    fn handle_recording(&self) {
        let samples = &self.downsampled_buffer[0][..self.downsampled_frames];
        if let Some(ref tx) = self.tx_audio {
            let mut block = AudioBlock::with_capacity(samples.len() * 2);
            for &sample in samples.iter() {
                // Quantize to i16 and duplicate for WAV.
                let v = (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                block.push(v);
                block.push(v);
            }

            if let Err(e) = tx.try_send(block) {
                error!("Error sending audio block: {e}");
            }
        }
    }

    fn handle_tuner(&mut self) {
        if let Some(ref mut tuner) = self.tuner {
            let samples = &self.input_buffer[0];
            for &sample in samples.iter() {
                tuner.process_sample(sample);
            }

            self.tuner_update_counter += samples.len();
            if self.tuner_update_counter >= 2048 {
                self.tuner_update_counter = 0;
                if let Some(ref tx) = self.tx_tuner {
                    let info = tuner.get_tuner_info();
                    let _ = tx.try_send(info);
                }
            }
        }
    }
}

impl ProcessHandler for Processor {
    fn process(&mut self, _c: &Client, ps: &ProcessScope) -> Control {
        self.handle_messages();

        let n_frames = ps.n_frames() as usize;
        let input = self.audio_ports.read_input(ps);

        self.input_buffer[0].copy_from_slice(input);

        if self.tuner.is_some() {
            self.handle_tuner();
            self.audio_ports.silence_output(ps, n_frames);

            return Control::Continue;
        }

        if let Err(e) = self.process() {
            error!("Error during processing: {}", e);
            return Control::Continue;
        }

        self.audio_ports.write_output(
            ps,
            &self.downsampled_buffer[0][..self.downsampled_frames],
            n_frames,
        );
        self.handle_recording();

        Control::Continue
    }

    fn buffer_size(&mut self, client: &Client, frames: Frames) -> Control {
        let new_size = frames as usize;
        warn!("buffer_size changed to {new_size} frames");
        debug_stats(client);

        let buffer = &mut self.input_buffer[0];
        if buffer.len() != new_size {
            buffer.resize(new_size, 0.0);
            info!("Input buffer resized to {new_size}");
        }

        if let Err(e) = self.upsampler.set_chunk_size(new_size) {
            error!("Upsampler cannot grow to {new_size}: {e}");
            return Control::Quit;
        }
        self.upsampled_buffer = self.upsampler.output_buffer_allocate(true);

        let downsampler_size = new_size * self.oversample_factor as usize;
        if let Err(e) = self.downsampler.set_chunk_size(downsampler_size) {
            error!("Downsampler cannot grow to {downsampler_size}: {e}");
            return Control::Quit;
        }
        self.downsampled_buffer = self.downsampler.output_buffer_allocate(true);

        Control::Continue
    }
}

fn debug_stats(client: &Client) {
    let sample_rate = client.sample_rate() as f32;
    let buffer_frames = client.buffer_size() as f32;
    debug!(
        "Sample rate: {sample_rate}, Buffer frames: {buffer_frames}, Calls p/s: {}",
        sample_rate / buffer_frames
    );
}
