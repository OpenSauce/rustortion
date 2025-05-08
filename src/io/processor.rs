use crate::io::recorder::{AudioBlock, BLOCK_FRAMES};
use crate::sim::chain::AmplifierChain;
use crossbeam::channel::{Receiver, Sender};
use jack::{AudioIn, AudioOut, Client, Control, Port, ProcessScope};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

pub struct Processor {
    /// The *current* mutable chain used by the audio thread
    chain: Box<AmplifierChain>,
    /// Receiver for preset updates from the GUI
    rx_chain: Receiver<Box<AmplifierChain>>,
    tx_audio: Option<Sender<AudioBlock>>,
    in_port: Port<AudioIn>,
    out_l: Port<AudioOut>,
    out_r: Port<AudioOut>,
    upsampler: SincFixedIn<f32>,
    downsampler: SincFixedIn<f32>,
}

impl Processor {
    /// Construct with an initial default chain plus a receiver for future ones
    pub fn new_with_channel(
        client: &Client,
        rx_chain: Receiver<Box<AmplifierChain>>,
        tx_audio: Option<Sender<AudioBlock>>,
    ) -> Self {
        let in_port = client.register_port("in", AudioIn::default()).unwrap();
        let out_l = client.register_port("out_l", AudioOut::default()).unwrap();
        let out_r = client.register_port("out_r", AudioOut::default()).unwrap();

        let _ = client.connect_ports_by_name("system:capture_1", "rustortion:in");
        let _ = client.connect_ports_by_name("rustortion:out_l", "system:playback_1");
        let _ = client.connect_ports_by_name("rustortion:out_r", "system:playback_2");

        let channels = 1;
        let oversample_factor: f32 = 4.0;
        let max_chunk_size = 128;

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
            oversample_factor as f64,
            1.0,
            interp_params,
            max_chunk_size,
            channels,
        )
        .unwrap();

        let downsampler = SincFixedIn::<f32>::new(
            1.0 / oversample_factor as f64,
            1.0,
            down_interp_params,
            (max_chunk_size as f32 * oversample_factor) as usize,
            channels,
        )
        .unwrap();

        Self {
            chain: Box::new(AmplifierChain::new("Default")),
            rx_chain,
            tx_audio,
            in_port,
            out_l,
            out_r,
            upsampler,
            downsampler,
        }
    }

    pub fn into_process_handler(
        mut self,
    ) -> Box<dyn FnMut(&Client, &ProcessScope) -> Control + Send + 'static> {
        Box::new(move |_client: &Client, ps: &ProcessScope| -> Control {
            // Check for a new preset without blocking
            if let Ok(new_chain) = self.rx_chain.try_recv() {
                self.chain = new_chain;
            }

            let n_frames = ps.n_frames() as usize;
            let in_buf = self.in_port.as_slice(ps);

            let out_buf_l = self.out_l.as_mut_slice(ps);
            let out_buf_r = self.out_r.as_mut_slice(ps);

            // --------------- DSP ---------------

            // Upsample
            let input_frames = vec![in_buf.to_vec()];
            let mut upsampled = match self.upsampler.process(&input_frames, None) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Upsampler error: {e}");
                    vec![Vec::new()]
                }
            };

            // Process
            let chain = self.chain.as_mut();
            for s in &mut upsampled[0] {
                *s = chain.process(*s); // ✅ no move, just &mut borrow
            }

            // Downsample
            let downsampled = match self.downsampler.process(&upsampled, None) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Downsampler error: {e}");
                    vec![Vec::new()]
                }
            };

            let final_samples = &downsampled[0];
            let frames_to_copy = final_samples.len().min(n_frames);

            for i in 0..n_frames {
                let out_sample = if i < frames_to_copy {
                    final_samples[i]
                } else {
                    0.0
                };
                out_buf_l[i] = out_sample;
                out_buf_r[i] = out_sample;
            }

            // Send to recorder (non‑blocking)
            if let Some(ref tx) = self.tx_audio {
                let mut block = AudioBlock::with_capacity(BLOCK_FRAMES * 2);
                for &s in final_samples.iter().take(BLOCK_FRAMES) {
                    let v = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                    block.extend_from_slice(&[v, v]);
                }
                let _ = tx.try_send(block);
            }

            Control::Continue
        })
    }
}
