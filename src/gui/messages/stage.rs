use crate::sim::stages::{
    clipper::ClipperType, filter::FilterType, poweramp::PowerAmpType, tonestack::ToneStackModel,
};

#[derive(Debug, Clone)]
pub enum StageMessage {
    Filter(FilterMessage),
    Preamp(PreampMessage),
    Compressor(CompressorMessage),
    ToneStack(ToneStackMessage),
    PowerAmp(PowerAmpMessage),
    Level(LevelMessage),
    NoiseGate(NoiseGateMessage),
}

#[derive(Debug, Clone)]
pub enum NoiseGateMessage {
    ThresholdChanged(f32),
    RatioChanged(f32),
    AttackChanged(f32),
    HoldChanged(f32),
    ReleaseChanged(f32),
}

#[derive(Debug, Clone)]
pub enum FilterMessage {
    TypeChanged(FilterType),
    CutoffChanged(f32),
    ResonanceChanged(f32),
}

#[derive(Debug, Clone)]
pub enum PreampMessage {
    GainChanged(f32),
    BiasChanged(f32),
    ClipperChanged(ClipperType),
}

#[derive(Debug, Clone)]
pub enum CompressorMessage {
    ThresholdChanged(f32),
    RatioChanged(f32),
    AttackChanged(f32),
    ReleaseChanged(f32),
    MakeupChanged(f32),
}

#[derive(Debug, Clone)]
pub enum ToneStackMessage {
    ModelChanged(ToneStackModel),
    BassChanged(f32),
    MidChanged(f32),
    TrebleChanged(f32),
    PresenceChanged(f32),
}

#[derive(Debug, Clone)]
pub enum PowerAmpMessage {
    TypeChanged(PowerAmpType),
    DriveChanged(f32),
    SagChanged(f32),
}

#[derive(Debug, Clone)]
pub enum LevelMessage {
    GainChanged(f32),
}
