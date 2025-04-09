use crate::amp::Amp;
use hound::WavWriter;
use jack::{AudioIn, AudioOut, Client, Control, Port, ProcessScope};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex, MutexGuard};

pub type RecordingWriter = Option<Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>>;

pub struct Processor {
    amp: Arc<Mutex<Amp>>,
    writer: RecordingWriter,
    in_port: Port<AudioIn>,
    out_l: Port<AudioOut>,
    out_r: Port<AudioOut>,
    upsampler: SincFixedIn<f32>,
    downsampler: SincFixedIn<f32>,
}

impl Processor {
    pub fn new(client: &Client, amp: Arc<Mutex<Amp>>, recording: bool) -> (Self, RecordingWriter) {
        let in_port = client.register_port("in", AudioIn).unwrap();
        let out_l = client.register_port("out_l", AudioOut).unwrap();
        let out_r = client.register_port("out_r", AudioOut).unwrap();

        let _ = client.connect_ports_by_name("system:capture_1", "rustortion:in");
        let _ = client.connect_ports_by_name("rustortion:out_l", "system:playback_1");
        let _ = client.connect_ports_by_name("rustortion:out_r", "system:playback_2");

        let sample_rate = client.sample_rate() as f32;

        let writer = if recording {
            let spec = hound::WavSpec {
                channels: 2,
                sample_rate: sample_rate as u32,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let filename = format!(
                "./recordings/recording_{}.wav",
                chrono::Local::now().format("%Y%m%d_%H%M%S")
            );
            println!("Recording to: {}", filename);
            let writer = hound::WavWriter::create(filename, spec).unwrap();
            Some(Arc::new(Mutex::new(Some(writer))))
        } else {
            None
        };

        // ---------------------------------------
        // Initialize rubato resamplers
        // We'll assume mono input (1 channel).
        // For stereo in/out, you'd set `channels = 2` and handle that carefully.
        // ---------------------------------------
        let channels = 1;
        let oversample_factor: f32 = 2.0;

        // The JACK buffer size can vary, but often is consistent (e.g., 128 or 256).
        // We'll guess an upper bound chunk size for rubato.
        // You may need to experiment or query the client for frames per period.
        let max_chunk_size = 64;

        // Common interpolation parameters:
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
            max_chunk_size as usize, // max upsampled chunk size
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

        (
            Self {
                amp,
                writer: writer.clone(),
                in_port,
                out_l,
                out_r,
                upsampler,
                downsampler,
            },
            writer,
        )
    }

    pub fn into_process_handler(
        self,
    ) -> impl FnMut(&Client, &ProcessScope) -> Control + Send + 'static {
        let Processor {
            amp,
            writer,
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

            {
                let mut amp_guard: MutexGuard<'_, Amp> = amp.lock().unwrap();
                let upsampled_channel = &mut upsampled[0];
                for sample in upsampled_channel.iter_mut() {
                    *sample = amp_guard.process_sample(*sample);
                }
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

            if let Some(mut writer_mutex) = writer.as_ref().map(|w| w.lock().unwrap()) {
                if let Some(ref mut writer) = *writer_mutex {
                    for &s in final_samples.iter().take(frames_to_copy) {
                        let sample_i16 =
                            (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                        writer.write_sample(sample_i16).unwrap(); // left
                        writer.write_sample(sample_i16).unwrap(); // right
                    }
                }
            }

            Control::Continue
        }
    }
}
