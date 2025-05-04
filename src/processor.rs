use crate::amp::Amp;
use crate::recorder::{AudioBlock, BLOCK_FRAMES};
use crossbeam::channel::Sender;
use jack::{AudioIn, AudioOut, Client, Control, Port, ProcessScope};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

pub struct Processor {
    amp: Amp,
    tx: Option<Sender<AudioBlock>>,
    in_port: Port<AudioIn>,
    out_l: Port<AudioOut>,
    out_r: Port<AudioOut>,
    upsampler: SincFixedIn<f32>,
    downsampler: SincFixedIn<f32>,
}

impl Processor {
    pub fn new(client: &Client, amp: Amp, tx: Option<Sender<AudioBlock>>) -> Self {
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
            oversample_factor as f64, // The resample ratio
            1.0,                      // Tolerance
            interp_params,
            max_chunk_size as usize,
            channels,
        )
        .unwrap();

        let downsampler = SincFixedIn::<f32>::new(
            1.0_f64 / oversample_factor as f64,
            1.0,
            down_interp_params,
            (max_chunk_size as f32 * oversample_factor) as usize,
            channels,
        )
        .unwrap();

        Self {
            amp,
            tx,
            in_port,
            out_l,
            out_r,
            upsampler,
            downsampler,
        }
    }

    pub fn into_process_handler(
        self,
    ) -> impl FnMut(&Client, &ProcessScope) -> Control + Send + 'static {
        let Processor {
            mut amp,
            tx,
            in_port,
            mut out_l,
            mut out_r,
            mut upsampler,
            mut downsampler,
        } = self;

        move |_client: &Client, ps: &ProcessScope| -> Control {
            let n_frames = ps.n_frames() as usize; // frames in this callback
            let in_buf = in_port.as_slice(ps);

            let out_buf_l = out_l.as_mut_slice(ps);
            let out_buf_r = out_r.as_mut_slice(ps);

            let mut input_frames = vec![Vec::with_capacity(n_frames); 1];
            input_frames[0].extend_from_slice(in_buf);

            let mut upsampled = match upsampler.process(&input_frames, None) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Upsampler error: {e}");
                    vec![Vec::new()]
                }
            };

            if upsampled[0].is_empty() {
                eprintln!("Upsampler returned an empty buffer");
            }

            let upsampled_channel = &mut upsampled[0];
            for sample in upsampled_channel.iter_mut() {
                *sample = amp.process_sample(*sample);
            }

            let downsampled = match downsampler.process(&upsampled, None) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Downsampler error: {e}");
                    vec![Vec::new()]
                }
            };

            if downsampled[0].is_empty() {
                eprintln!("Downsampler returned an empty buffer");
            }

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

            if let Some(ref tx) = tx {
                let mut block = AudioBlock::with_capacity(BLOCK_FRAMES * 2);
                for &s in final_samples.iter().take(BLOCK_FRAMES) {
                    let v = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                    block.extend_from_slice(&[v, v]);
                }
                let _ = tx.try_send(block); // never blocks
            }

            Control::Continue
        }
    }
}
