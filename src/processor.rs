use crate::amp::Amp;
use jack::{AudioIn, AudioOut, Client, Control, Port, ProcessScope};
use std::sync::{Arc, Mutex};

pub type RecordingWriter =
    Option<Arc<Mutex<Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>>>>;

pub struct Processor {
    amp: Arc<Mutex<Amp>>,
    writer: RecordingWriter,
    in_port: Port<AudioIn>,
    out_l: Port<AudioOut>,
    out_r: Port<AudioOut>,
}

impl Processor {
    pub fn new(
        client: &Client,
        gain: f32,
        recording: bool,
    ) -> (Self, Arc<Mutex<Amp>>, RecordingWriter) {
        let in_port = client.register_port("in", AudioIn).unwrap();
        let out_l = client.register_port("out_l", AudioOut).unwrap();
        let out_r = client.register_port("out_r", AudioOut).unwrap();

        let _ = client.connect_ports_by_name("system:capture_1", "rustortion:in");
        let _ = client.connect_ports_by_name("rustortion:out_l", "system:playback_1");
        let _ = client.connect_ports_by_name("rustortion:out_r", "system:playback_2");

        let sample_rate = client.sample_rate() as f32;
        let amp = Arc::new(Mutex::new(Amp::new(gain, sample_rate)));

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
            let writer = hound::WavWriter::create(filename, spec).unwrap();
            Some(Arc::new(Mutex::new(Some(writer))))
        } else {
            None
        };

        (
            Self {
                amp: Arc::clone(&amp),
                writer: writer.clone(),
                in_port,
                out_l,
                out_r,
            },
            amp,
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
        } = self;

        move |_: &Client, ps: &ProcessScope| -> Control {
            let in_buf = in_port.as_slice(ps);
            let out_buf_l = out_l.as_mut_slice(ps);
            let out_buf_r = out_r.as_mut_slice(ps);

            let mut writer_guard = writer.as_ref().map(|w| w.lock().unwrap());

            for ((l, r), input) in out_buf_l
                .iter_mut()
                .zip(out_buf_r.iter_mut())
                .zip(in_buf.iter())
            {
                let out = amp.lock().unwrap().process_sample(*input);
                *l = out;
                *r = out;

                if let Some(writer_mutex) = &mut writer_guard {
                    if let Some(ref mut writer) = **writer_mutex {
                        let sample =
                            (out * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                        writer.write_sample(sample).unwrap();
                        writer.write_sample(sample).unwrap();
                    }
                }
            }

            Control::Continue
        }
    }
}
