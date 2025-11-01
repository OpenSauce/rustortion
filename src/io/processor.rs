use std::path::Path;

use crate::io::recorder::AudioBlock;
use crate::ir::cabinet::IrCabinet;
use crate::sim::chain::AmplifierChain;
use crate::sim::tuner::{Tuner, TunerInfo};
use anyhow::{Context, Result};
use crossbeam::channel::{Receiver, Sender};
use jack::{AudioIn, AudioOut, Client, Control, Frames, Port, ProcessHandler, ProcessScope};
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

const CHANNELS: usize = 1;
const MAX_BLOCK_SIZE: usize = 8192;

pub struct Processor {
    /// Amplifier chain, used for processing amp simulations on the input.
    chain: Box<AmplifierChain>,
    /// IR Cabinet processor
    ir_cabinet: Option<IrCabinet>,
    /// Channel for updating the amplifier chain.
    rx_updates: Receiver<ProcessorMessage>,
    /// Optional recorder channel.
    tx_audio: Option<Sender<AudioBlock>>,
    in_port: Port<AudioIn>,
    out_port_left: Port<AudioOut>,
    out_port_right: Port<AudioOut>,
    upsampler: SincFixedIn<f32>,
    downsampler: SincFixedIn<f32>,
    /// Reusable buffer for input frames.
    input_buffer: Vec<Vec<f32>>,
    /// Reusable buffer for upsampled frames.
    upsampled_buffer: Vec<Vec<f32>>,
    /// Reusable buffer for downsampled frames.
    downsampled_buffer: Vec<Vec<f32>>,
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
        let in_port = client
            .register_port("in_port", AudioIn::default())
            .context("failed to register in port")?;
        let out_port_left = client
            .register_port("out_port_left", AudioOut::default())
            .context("failed to register out port left")?;
        let out_port_right = client
            .register_port("out_port_right", AudioOut::default())
            .context("failed to register out port right")?;

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

        let input_buffer = vec![Vec::with_capacity(buffer_size)];
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
            in_port,
            out_port_left,
            out_port_right,
            upsampler,
            downsampler,
            input_buffer,
            upsampled_buffer,
            downsampled_buffer,
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

    fn handle_recording(&self, buffer: &[f32]) {
        if let Some(ref tx) = self.tx_audio {
            let mut block = AudioBlock::with_capacity(buffer.len() * 2);
            for &s in buffer.iter() {
                let v = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                block.push(v);
                block.push(v);
            }

            if let Err(e) = tx.try_send(block) {
                error!("Error sending audio block: {e}");
            }
        }
    }
}

impl ProcessHandler for Processor {
    fn process(&mut self, _c: &Client, ps: &ProcessScope) -> Control {
        // Handle messages received from the main thread.
        self.handle_messages();

        let n_frames = ps.n_frames() as usize;
        let input = self.in_port.as_slice(ps);

        if let Some(ref mut tuner) = self.tuner {
            for &sample in input.iter() {
                tuner.process_sample(sample);
            }

            self.tuner_update_counter += n_frames;
            if self.tuner_update_counter >= 2048 {
                self.tuner_update_counter = 0;
                if let Some(ref tx) = self.tx_tuner {
                    let info = tuner.get_tuner_info();
                    let _ = tx.try_send(info);
                }
            }

            let out_left = self.out_port_left.as_mut_slice(ps);
            let out_right = self.out_port_right.as_mut_slice(ps);
            out_left[..n_frames].fill(0.0);
            out_right[..n_frames].fill(0.0);

            return Control::Continue;
        }

        self.input_buffer[0].clear();

        debug_assert!(
            self.input_buffer[0].capacity() >= n_frames,
            "input_buffer too small; buffer_size callback missing an allocation"
        );

        self.input_buffer[0].extend_from_slice(input);

        let (_, upsampled_frames) = match self.upsampler.process_into_buffer(
            &self.input_buffer,
            &mut self.upsampled_buffer,
            None,
        ) {
            Ok(data) => data,
            Err(e) => {
                error!("Upsampler error: {e}");
                return Control::Continue;
            }
        };

        let chain = self.chain.as_mut();
        for s in &mut self.upsampled_buffer[0][..upsampled_frames] {
            *s = chain.process(*s);
        }

        let (_, downsampled_frames) = match self.downsampler.process_into_buffer(
            &self.upsampled_buffer,
            &mut self.downsampled_buffer,
            None,
        ) {
            Ok(data) => data,
            Err(e) => {
                error!("Downsampler error: {e}");
                return Control::Continue;
            }
        };

        let final_samples = &mut self.downsampled_buffer[0][..downsampled_frames];

        if let Some(ref mut cab) = self.ir_cabinet {
            cab.process_block(final_samples);
        }

        let frames_to_copy = final_samples.len().min(n_frames);

        let out_buffer_left = self.out_port_left.as_mut_slice(ps);
        let out_buffer_right = self.out_port_right.as_mut_slice(ps);

        out_buffer_left[..frames_to_copy].copy_from_slice(&final_samples[..frames_to_copy]);
        out_buffer_right[..frames_to_copy].copy_from_slice(&final_samples[..frames_to_copy]);
        for i in frames_to_copy..n_frames {
            out_buffer_left[i] = 0.0;
            out_buffer_right[i] = 0.0;
        }

        self.handle_recording(&self.downsampled_buffer[0][..downsampled_frames]);

        Control::Continue
    }

    fn buffer_size(&mut self, client: &Client, frames: Frames) -> Control {
        let new_size = frames as usize;
        warn!("buffer_size changed to {new_size} frames");
        debug_stats(client);

        let buffer = &mut self.input_buffer[0];
        if buffer.capacity() < new_size {
            buffer.reserve_exact(new_size - buffer.len());
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
