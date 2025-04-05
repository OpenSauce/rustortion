use jack::{AudioIn, AudioOut, Client, ClientOptions, Control, ProcessScope};
use std::{env, thread, time::Duration};

fn main() {
    // Set latency env vars early
    unsafe {
        env::set_var("PIPEWIRE_LATENCY", "64/48000");
        env::set_var("JACK_PROMISCUOUS_SERVER", "pipewire");
    }

    // Optional gain factor from CLI
    let gain: f32 = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.5); // default to 1.5x gain

    let (client, _status) = Client::new("rustortion", ClientOptions::NO_START_SERVER).unwrap();
    let in_port = client.register_port("in", AudioIn).unwrap();
    let mut out_l = client.register_port("out_l", AudioOut).unwrap();
    let mut out_r = client.register_port("out_r", AudioOut).unwrap();

    // Auto-connect
    let _ = client.connect_ports_by_name("system:capture_1", "rustortion:in");
    let _ = client.connect_ports_by_name("rustortion:out_l", "system:playback_1");
    let _ = client.connect_ports_by_name("rustortion:out_r", "system:playback_2");

    // DSP process callback
    let process = move |_: &jack::Client, ps: &ProcessScope| -> Control {
        let in_buf = in_port.as_slice(ps);
        let out_buf_l = out_l.as_mut_slice(ps);
        let out_buf_r = out_r.as_mut_slice(ps);

        for ((l, r), input) in out_buf_l
            .iter_mut()
            .zip(out_buf_r.iter_mut())
            .zip(in_buf.iter())
        {
            // Apply gain and clamp to avoid hard clipping
            let boosted = *input * gain;
            let clamped = boosted.clamp(-1.0, 1.0);
            *l = clamped;
            *r = clamped;
        }

        Control::Continue
    };

    let _active_client = client
        .activate_async(Notifications, jack::ClosureProcessHandler::new(process))
        .unwrap();

    println!("Stereo output active with gain {:.2}!", gain);

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

struct Notifications;
impl jack::NotificationHandler for Notifications {}
