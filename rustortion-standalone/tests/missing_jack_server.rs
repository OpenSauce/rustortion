//! A missing JACK server is a documented, expected condition. Booting without
//! one must return an error the caller can report, never a panic (REV-11).

use rustortion::gui::AmplifierApp;
use rustortion::settings::Settings;

#[test]
fn missing_jack_server_is_reported_as_an_error() {
    // Point both the jackd2 and the pipewire-jack clients at a server that
    // cannot exist, and forbid autostart, so connecting always fails — even on
    // a developer machine that does have JACK/PipeWire running.
    //
    // Safe: this is the only test in this integration-test binary, so no other
    // thread is reading the environment concurrently.
    unsafe {
        std::env::set_var("JACK_NO_START_SERVER", "1");
        std::env::set_var("JACK_DEFAULT_SERVER", "rustortion-no-such-jack-server");
        std::env::set_var("PIPEWIRE_REMOTE", "rustortion-no-such-jack-server");
    }

    let error = AmplifierApp::new(Settings::default())
        .err()
        .expect("expected an error when no JACK server is reachable");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("JACK server not running"),
        "expected an actionable message, got: {rendered}"
    );
}
