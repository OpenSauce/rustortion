use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use jack::{AsyncClient, Client, ClientOptions};
use log::{error, info, warn};

use std::path::Path;

use crate::audio::engine::Engine;
use crate::audio::engine::EngineHandle;
use crate::audio::jack::{NotificationHandler, ProcessHandler};
use crate::audio::peak_meter::{PeakMeter, PeakMeterHandle};
use crate::audio::samplers::Samplers;
use crate::ir::cabinet::IrCabinet;
use crate::metronome::Metronome;
use crate::settings::{AudioSettings, Settings};
use crate::tuner::{Tuner, TunerHandle};

pub struct Manager {
    active_client: AsyncClient<NotificationHandler, ProcessHandler>,
    current_settings: Settings,
    tuner_handle: TunerHandle,
    engine_handle: EngineHandle,
    peak_meter_handle: PeakMeterHandle,
    xrun_count: Arc<AtomicU64>,
    available_irs: Vec<String>,
}

impl Manager {
    pub fn new(settings: Settings) -> Result<Self> {
        let (client, _) = Client::new("rustortion", ClientOptions::NO_START_SERVER)
            .context("failed to create JACK client")?;

        let sample_rate = client.sample_rate() as usize;
        let buffer_size = client.buffer_size() as usize;

        let (tuner, tuner_handle) = Tuner::new(sample_rate);
        let (peak_meter, peak_meter_handle) = PeakMeter::new(sample_rate);
        let samplers = Samplers::new(
            buffer_size,
            settings.audio.oversampling_factor.into(),
            sample_rate,
        )?;
        let mut metronome = Metronome::new(120.0, sample_rate);
        metronome.load_wav_file("click.wav");

        let ir_cabinet = match IrCabinet::new(Path::new(&settings.ir_dir), sample_rate) {
            Ok(cab) => Some(cab),
            Err(e) => {
                warn!("Failed to load IR Cabinet: {}", e);
                None
            }
        };

        let available_irs = ir_cabinet
            .as_ref()
            .map(|c| c.available_ir_names())
            .unwrap_or_default();

        let (engine, engine_handle) =
            Engine::new(tuner, samplers, ir_cabinet, peak_meter, metronome)?;
        let jack_handler =
            ProcessHandler::new(&client, engine).context("failed to create process handler")?;

        let xrun_count = Arc::new(AtomicU64::new(0));
        let notification_handler = NotificationHandler::new(xrun_count.clone());

        let active_client = client
            .activate_async(notification_handler, jack_handler)
            .context("failed to activate async client")?;

        let mut manager = Self {
            active_client,
            current_settings: settings.clone(),
            tuner_handle,
            engine_handle,
            peak_meter_handle,
            xrun_count,
            available_irs,
        };

        manager.connect_ports(&settings.audio);

        Ok(manager)
    }

    /// Connect audio ports based on settings
    fn connect_ports(&mut self, settings: &AudioSettings) {
        let client = self.active_client.as_client();

        try_connect(client, &settings.input_port, "rustortion:in_port");
        try_connect(
            client,
            "rustortion:out_port_left",
            &settings.output_left_port,
        );
        try_connect(
            client,
            "rustortion:out_port_right",
            &settings.output_right_port,
        );
        try_connect(
            client,
            "rustortion:metronome_out_port",
            &settings.metronome_out_port,
        );
    }

    pub fn engine(&self) -> &EngineHandle {
        &self.engine_handle
    }

    pub fn tuner(&self) -> &TunerHandle {
        &self.tuner_handle
    }

    pub fn peak_meter(&self) -> &PeakMeterHandle {
        &self.peak_meter_handle
    }

    pub fn xrun_count(&self) -> u64 {
        self.xrun_count.load(Ordering::Relaxed)
    }

    pub fn cpu_load(&self) -> f32 {
        self.active_client.as_client().cpu_load()
    }

    /// Reconnect with new settings
    pub fn apply_settings(&mut self, new_settings: AudioSettings) -> Result<()> {
        info!("Applying new audio settings");

        // Disconnect existing connections
        self.disconnect_all();

        // Update settings
        self.current_settings.audio = new_settings.clone();

        self.connect_ports(&new_settings);

        Ok(())
    }

    /// Disconnect all audio connections
    pub fn disconnect_all(&self) {
        let client = self.active_client.as_client();

        try_disconnect(client, "rustortion:in_port");
        try_disconnect(client, "rustortion:out_port_left");
        try_disconnect(client, "rustortion:out_port_right");
        try_disconnect(client, "rustortion:metronome_out_port");
    }

    /// Get available input ports
    pub fn get_available_inputs(&self) -> Vec<String> {
        self.active_client
            .as_client()
            .ports(None, Some("audio"), jack::PortFlags::IS_OUTPUT)
            .into_iter()
            .filter(|p| !p.starts_with("rustortion:"))
            .collect()
    }

    /// Get available output ports
    pub fn get_available_outputs(&self) -> Vec<String> {
        self.active_client
            .as_client()
            .ports(None, Some("audio"), jack::PortFlags::IS_INPUT)
            .into_iter()
            .filter(|p| !p.starts_with("rustortion:"))
            .collect()
    }

    // Get available IR paths
    pub fn get_available_irs(&self) -> Vec<String> {
        self.available_irs.clone()
    }

    pub fn sample_rate(&self) -> usize {
        self.active_client.as_client().sample_rate() as usize
    }

    pub fn buffer_size(&self) -> usize {
        self.active_client.as_client().buffer_size() as usize
    }
}

fn try_connect(client: &Client, src: &str, dst: &str) {
    if let Err(e) = client.connect_ports_by_name(src, dst) {
        warn!("Failed to connect '{src}' -> '{dst}': {e}");
    } else {
        info!("Connected: {src} -> {dst}");
    }
}

fn try_disconnect(client: &Client, port_name: &str) {
    if let Some(port) = client.port_by_name(port_name) {
        client.disconnect(&port).unwrap_or_else(|e| {
            error!("Failed to disconnect {port_name}: {e}");
        });
    }
}
