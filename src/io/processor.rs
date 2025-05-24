use crate::io::recorder::{AudioBlock, BLOCK_FRAMES};
use crate::sim::chain::AmplifierChain;
use anyhow::{Context, Result};
use crossbeam::channel::{Receiver, Sender};
use jack::{AudioIn, AudioOut, Client, Control, Frames, Port, ProcessHandler, ProcessScope};
use log::{error, info};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

const CHANNELS: usize = 1;
const OVERSAMPLE_FACTOR: f64 = 4.0;

pub struct Processor {
    /// Amplifier chain, used for processing amp simulations on the input.
    chain: Box<AmplifierChain>,
    /// Channel for updating the amplifier chain.
    rx_chain: Receiver<Box<AmplifierChain>>,
    /// Optional recorder channel.
    tx_audio: Option<Sender<AudioBlock>>,
    in_port: Port<AudioIn>,
    out_port_left: Port<AudioOut>,
    out_port_right: Port<AudioOut>,
    upsampler: SincFixedIn<f32>,
    downsampler: SincFixedIn<f32>,
    /// Reusable buffer for input frames.
    input_buffer: Vec<Vec<f32>>,
    /// Resuable buffer for upsampled frames.
    upsampled_buffer: Vec<Vec<f32>>,
    /// Resuable buffer for downsampled frames.
    downsampled_buffer: Vec<Vec<f32>>,
}

impl Processor {
    pub fn new(
        client: &Client,
        rx_chain: Receiver<Box<AmplifierChain>>,
        tx_audio: Option<Sender<AudioBlock>>,
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

        client
            .connect_ports_by_name("system:capture_1", "rustortion:in_port")
            .context("failed to connect to in port")?;
        client
            .connect_ports_by_name("rustortion:out_port_left", "system:playback_1")
            .context("faled to connect to out port left")?;
        client
            .connect_ports_by_name("rustortion:out_port_right", "system:playback_2")
            .context("failed to connect to out port right")?;

        let max_chunk_size = client.buffer_size() as usize;
        info!("Max chunk size: {max_chunk_size}, oversample factor: {OVERSAMPLE_FACTOR}");

        let interp_params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Cubic,
            oversampling_factor: 160,
            window: WindowFunction::BlackmanHarris2,
        };
        let down_interp_params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Cubic,
            oversampling_factor: 160,
            window: WindowFunction::BlackmanHarris2,
        };

        let upsampler = SincFixedIn::<f32>::new(
            OVERSAMPLE_FACTOR,
            1.0,
            interp_params,
            max_chunk_size,
            CHANNELS,
        )
        .context("failed to create upsampler")?;

        let downsampler = SincFixedIn::<f32>::new(
            1.0 / OVERSAMPLE_FACTOR,
            1.0,
            down_interp_params,
            (max_chunk_size as f64 * OVERSAMPLE_FACTOR) as usize,
            CHANNELS,
        )
        .context("failed to create downsampler")?;

        let input_buffer = vec![Vec::with_capacity(client.buffer_size() as usize)];
        let upsampled_buffer = upsampler.output_buffer_allocate(true);
        let downsampled_buffer = downsampler.output_buffer_allocate(true);

        Ok(Self {
            chain: Box::new(AmplifierChain::new("Default")),
            rx_chain,
            tx_audio,
            in_port,
            out_port_left,
            out_port_right,
            upsampler,
            downsampler,
            input_buffer,
            upsampled_buffer,
            downsampled_buffer,
        })
    }
}

impl ProcessHandler for Processor {
    fn process(&mut self, _c: &Client, ps: &ProcessScope) -> Control {
        if let Ok(new_chain) = self.rx_chain.try_recv() {
            self.chain = new_chain;
            info!("Received new chain");
        }

        let n_frames = ps.n_frames() as usize;
        let input = self.in_port.as_slice(ps);

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

        let final_samples = &self.downsampled_buffer[0][..downsampled_frames];
        let frames_to_copy = final_samples.len().min(n_frames);

        let out_buffer_left = self.out_port_left.as_mut_slice(ps);
        let out_buffer_right = self.out_port_right.as_mut_slice(ps);

        out_buffer_left[..frames_to_copy].copy_from_slice(&final_samples[..frames_to_copy]);
        out_buffer_right[..frames_to_copy].copy_from_slice(&final_samples[..frames_to_copy]);
        for i in frames_to_copy..n_frames {
            out_buffer_left[i] = 0.0;
            out_buffer_right[i] = 0.0;
        }

        // If the recording channel is available, handle sending audio blocks to the recorder.
        if let Some(ref tx) = self.tx_audio {
            let mut block = AudioBlock::with_capacity(BLOCK_FRAMES * 2);
            for &s in final_samples.iter().take(BLOCK_FRAMES) {
                let v = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                block.extend_from_slice(&[v, v]);
            }

            if let Err(e) = tx.try_send(block) {
                error!("Error sending audio block: {e}");
            }
        }

        Control::Continue
    }

    fn buffer_size(&mut self, _c: &Client, frames: Frames) -> Control {
        // TODO: Handle buffer size changes gracefully.
        let needed = frames as usize;
        let cap = self.input_buffer[0].capacity();

        if cap < needed {
            self.input_buffer[0].reserve_exact(needed - cap);
        }
        Control::Continue
    }
}
