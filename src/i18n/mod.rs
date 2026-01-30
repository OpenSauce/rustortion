use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};

static CURRENT_LANGUAGE: AtomicU8 = AtomicU8::new(0); // 0 = English, 1 = ZhCn

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(alias = "en")]
    English,
    #[serde(rename = "zh-CN", alias = "Chinese")]
    ZhCn,
}

impl Language {
    fn to_u8(self) -> u8 {
        match self {
            Language::English => 0,
            Language::ZhCn => 1,
        }
    }

    fn from_u8(val: u8) -> Self {
        match val {
            1 => Language::ZhCn,
            _ => Language::English,
        }
    }
}

impl Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::English => write!(f, "English"),
            Language::ZhCn => write!(f, "中文（简体）"),
        }
    }
}

pub const LANGUAGES: &[Language] = &[Language::English, Language::ZhCn];

/// Set the current language globally
pub fn set_language(lang: Language) {
    CURRENT_LANGUAGE.store(lang.to_u8(), Ordering::SeqCst);
}

/// Get the current language
pub fn get_language() -> Language {
    Language::from_u8(CURRENT_LANGUAGE.load(Ordering::SeqCst))
}

/// Get the current translations based on the global language setting
#[inline]
pub fn translations() -> &'static Translations {
    Translations::for_language(get_language())
}

/// Macro to access translations without importing
/// Usage: `tr!().field_name` or `tr!(field_name)`
#[macro_export]
macro_rules! tr {
    () => {
        $crate::i18n::translations()
    };
    ($field:ident) => {
        $crate::i18n::translations().$field
    };
}

/// All translatable strings in the application
#[derive(Debug, Clone)]
pub struct Translations {
    // Top bar buttons
    pub midi: &'static str,
    pub tuner: &'static str,
    pub settings: &'static str,

    // Audio Settings dialog
    pub audio_settings: &'static str,
    pub input_port: &'static str,
    pub output_left_port: &'static str,
    pub output_right_port: &'static str,
    pub buffer_size_requested: &'static str,
    pub sample_rate_requested: &'static str,
    pub oversampling_factor: &'static str,
    pub actual_latency: &'static str,
    pub changes_require_restart: &'static str,
    pub jack_server_status: &'static str,
    pub sample_rate: &'static str,
    pub buffer_size: &'static str,
    pub jack_different_settings: &'static str,
    pub refresh_ports: &'static str,
    pub cancel: &'static str,
    pub apply: &'static str,
    pub language: &'static str,

    // Tuner dialog
    pub tuner_title: &'static str,
    pub in_tune: &'static str,
    pub adjust: &'static str,
    pub play_a_note: &'static str,
    pub close: &'static str,
    pub flat: &'static str,
    pub sharp: &'static str,

    // MIDI dialog
    pub midi_settings: &'static str,
    pub controller: &'static str,
    pub connected: &'static str,
    pub not_connected: &'static str,
    pub device: &'static str,
    pub select_midi_controller: &'static str,
    pub disconnect: &'static str,
    pub input_mappings: &'static str,
    pub add_mapping: &'static str,
    pub press_midi_device: &'static str,
    pub captured: &'static str,
    pub assign_to: &'static str,
    pub select_preset: &'static str,
    pub confirm_mapping: &'static str,
    pub no_mappings_configured: &'static str,
    pub debug_log: &'static str,
    pub no_midi_messages: &'static str,
    pub refresh_controllers: &'static str,

    // Control bar
    pub add_stage: &'static str,
    pub stop_recording: &'static str,
    pub start_recording: &'static str,
    pub recording: &'static str,

    // IR Cabinet control
    pub cabinet_ir: &'static str,
    pub ir: &'static str,
    pub bypassed: &'static str,
    pub gain: &'static str,
    pub active: &'static str,
    pub no_ir_loaded: &'static str,

    // Preset bar
    pub preset: &'static str,
    pub overwrite_preset: &'static str,
    pub yes: &'static str,
    pub no: &'static str,
    pub preset_name_placeholder: &'static str,
    pub save: &'static str,
    pub save_as: &'static str,
    pub update: &'static str,
    pub delete: &'static str,

    // Stage names
    pub stage_filter: &'static str,
    pub stage_preamp: &'static str,
    pub stage_compressor: &'static str,
    pub stage_tone_stack: &'static str,
    pub stage_power_amp: &'static str,
    pub stage_level: &'static str,
    pub stage_noise_gate: &'static str,
    pub stage_multiband_saturator: &'static str,

    // Stage parameters
    pub clipper: &'static str,
    pub bias: &'static str,
    pub threshold: &'static str,
    pub ratio: &'static str,
    pub attack: &'static str,
    pub release: &'static str,
    pub makeup: &'static str,
    pub model: &'static str,
    pub bass: &'static str,
    pub mid: &'static str,
    pub treble: &'static str,
    pub presence: &'static str,
    pub type_label: &'static str,
    pub drive: &'static str,
    pub sag: &'static str,
    pub cutoff: &'static str,
    pub hold: &'static str,
    pub low_band: &'static str,
    pub mid_band: &'static str,
    pub high_band: &'static str,
    pub low_freq: &'static str,
    pub high_freq: &'static str,
    pub level: &'static str,
    pub crossover: &'static str,

    // Filter types
    pub filter_highpass: &'static str,
    pub filter_lowpass: &'static str,

    // Clipper types
    pub clipper_soft: &'static str,
    pub clipper_medium: &'static str,
    pub clipper_hard: &'static str,
    pub clipper_asymmetric: &'static str,
    pub clipper_class_a: &'static str,

    // Power amp types
    pub poweramp_class_a: &'static str,
    pub poweramp_class_ab: &'static str,
    pub poweramp_class_b: &'static str,

    // Tone stack models
    pub tonestack_modern: &'static str,
    pub tonestack_british: &'static str,
    pub tonestack_american: &'static str,
    pub tonestack_flat: &'static str,

    // Misc UI labels
    pub output: &'static str,
    pub samples: &'static str,
    pub requested: &'static str,
    pub hz: &'static str,
    pub db: &'static str,
    pub ms: &'static str,
}

impl Translations {
    pub fn for_language(lang: Language) -> &'static Self {
        match lang {
            Language::English => &EN,
            Language::ZhCn => &ZH_CN,
        }
    }
}

pub static EN: Translations = Translations {
    // Top bar buttons
    midi: "Midi",
    tuner: "Tuner",
    settings: "Settings",

    // Audio Settings dialog
    audio_settings: "Audio Settings",
    input_port: "Input Port:",
    output_left_port: "Output Left Port:",
    output_right_port: "Output Right Port:",
    buffer_size_requested: "Buffer Size* (requested):",
    sample_rate_requested: "Sample Rate* (requested):",
    oversampling_factor: "Oversampling Factor*:",
    actual_latency: "Actual Latency:",
    changes_require_restart: "* Changes require restart",
    jack_server_status: "JACK Server Status",
    sample_rate: "Sample Rate:",
    buffer_size: "Buffer Size:",
    jack_different_settings: "JACK is using different settings than requested. This may be controlled by PipeWire/JACK server configuration.",
    refresh_ports: "Refresh Ports",
    cancel: "Cancel",
    apply: "Apply",
    language: "Language:",

    // Tuner dialog
    tuner_title: "TUNER",
    in_tune: "IN TUNE",
    adjust: "ADJUST",
    play_a_note: "PLAY A NOTE",
    close: "Close",
    flat: "FLAT",
    sharp: "SHARP",

    // MIDI dialog
    midi_settings: "MIDI Settings",
    controller: "Controller",
    connected: "Connected",
    not_connected: "Not connected",
    device: "Device:",
    select_midi_controller: "Select a MIDI controller...",
    disconnect: "Disconnect",
    input_mappings: "Input Mappings",
    add_mapping: "Add Mapping",
    press_midi_device: "Press a button or move a control on your MIDI device...",
    captured: "Captured:",
    assign_to: "Assign to:",
    select_preset: "Select a preset...",
    confirm_mapping: "Confirm Mapping",
    no_mappings_configured: "No mappings configured",
    debug_log: "Debug Log",
    no_midi_messages: "No MIDI messages received yet",
    refresh_controllers: "Refresh Controllers",

    // Control bar
    add_stage: "Add Stage",
    stop_recording: "Stop Recording",
    start_recording: "Start Recording",
    recording: "Recording...",

    // IR Cabinet control
    cabinet_ir: "Cabinet IR",
    ir: "IR:",
    bypassed: "Bypassed",
    gain: "Gain",
    active: "Active:",
    no_ir_loaded: "No IR loaded",

    // Preset bar
    preset: "Preset:",
    overwrite_preset: "Overwrite",
    yes: "Yes",
    no: "No",
    preset_name_placeholder: "Preset name...",
    save: "Save",
    save_as: "Save As...",
    update: "Update",
    delete: "Delete",

    // Stage names
    stage_filter: "Filter",
    stage_preamp: "Preamp",
    stage_compressor: "Compressor",
    stage_tone_stack: "Tone Stack",
    stage_power_amp: "Power Amp",
    stage_level: "Level",
    stage_noise_gate: "Noise Gate",
    stage_multiband_saturator: "Multiband Saturator",

    // Stage parameters
    clipper: "Clipper:",
    bias: "Bias",
    threshold: "Threshold",
    ratio: "Ratio",
    attack: "Attack",
    release: "Release",
    makeup: "Makeup",
    model: "Model:",
    bass: "Bass",
    mid: "Mid",
    treble: "Treble",
    presence: "Presence",
    type_label: "Type:",
    drive: "Drive",
    sag: "Sag",
    cutoff: "Cutoff",
    hold: "Hold",
    low_band: "Low Band",
    mid_band: "Mid Band",
    high_band: "High Band",
    low_freq: "Low Crossover",
    high_freq: "High Crossover",
    level: "Level",
    crossover: "Crossover",

    // Filter types
    filter_highpass: "Highpass",
    filter_lowpass: "Lowpass",

    // Clipper types
    clipper_soft: "Soft Clipping",
    clipper_medium: "Medium Clipping",
    clipper_hard: "Hard Clipping",
    clipper_asymmetric: "Asymmetric Clipping",
    clipper_class_a: "Class A Tube Preamp",

    // Power amp types
    poweramp_class_a: "Class A",
    poweramp_class_ab: "Class AB",
    poweramp_class_b: "Class B",

    // Tone stack models
    tonestack_modern: "Modern",
    tonestack_british: "British",
    tonestack_american: "American",
    tonestack_flat: "Flat",

    // Misc UI labels
    output: "Output:",
    samples: "samples",
    requested: "requested:",
    hz: "Hz",
    db: "dB",
    ms: "ms",
};

pub static ZH_CN: Translations = Translations {
    // Top bar buttons
    midi: "MIDI",
    tuner: "调音器",
    settings: "设置",

    // Audio Settings dialog
    audio_settings: "音频设置",
    input_port: "输入端口:",
    output_left_port: "左输出端口:",
    output_right_port: "右输出端口:",
    buffer_size_requested: "缓冲区大小* (请求):",
    sample_rate_requested: "采样率* (请求):",
    oversampling_factor: "过采样倍数*:",
    actual_latency: "实际延迟:",
    changes_require_restart: "* 更改需要重启",
    jack_server_status: "JACK 服务器状态",
    sample_rate: "采样率:",
    buffer_size: "缓冲区大小:",
    jack_different_settings: "JACK 使用的设置与请求的不同。这可能由 PipeWire/JACK 服务器配置控制。",
    refresh_ports: "刷新端口",
    cancel: "取消",
    apply: "应用",
    language: "语言:",

    // Tuner dialog
    tuner_title: "调音器",
    in_tune: "已调准",
    adjust: "调整",
    play_a_note: "请弹奏",
    close: "关闭",
    flat: "偏低",
    sharp: "偏高",

    // MIDI dialog
    midi_settings: "MIDI 设置",
    controller: "控制器",
    connected: "已连接",
    not_connected: "未连接",
    device: "设备:",
    select_midi_controller: "选择 MIDI 控制器...",
    disconnect: "断开",
    input_mappings: "输入映射",
    add_mapping: "添加映射",
    press_midi_device: "请按下 MIDI 设备上的按钮或移动控制器...",
    captured: "已捕获:",
    assign_to: "分配到:",
    select_preset: "选择预设...",
    confirm_mapping: "确认映射",
    no_mappings_configured: "未配置映射",
    debug_log: "调试日志",
    no_midi_messages: "尚未收到 MIDI 消息",
    refresh_controllers: "刷新控制器",

    // Control bar
    add_stage: "添加级",
    stop_recording: "停止录音",
    start_recording: "开始录音",
    recording: "录音中...",

    // IR Cabinet control
    cabinet_ir: "箱体脉冲响应",
    ir: "IR:",
    bypassed: "已旁通",
    gain: "增益",
    active: "当前:",
    no_ir_loaded: "未加载 IR",

    // Preset bar
    preset: "预设:",
    overwrite_preset: "覆盖",
    yes: "是",
    no: "否",
    preset_name_placeholder: "预设名称...",
    save: "保存",
    save_as: "另存为...",
    update: "更新",
    delete: "删除",

    // Stage names
    stage_filter: "滤波器",
    stage_preamp: "前级放大",
    stage_compressor: "压缩器",
    stage_tone_stack: "音色堆栈",
    stage_power_amp: "功率放大",
    stage_level: "电平",
    stage_noise_gate: "噪声门",
    stage_multiband_saturator: "多段饱和器",

    // Stage parameters
    clipper: "削波器:",
    bias: "偏置",
    threshold: "阈值",
    ratio: "比率",
    attack: "启动",
    release: "释放",
    makeup: "补偿",
    model: "模型:",
    bass: "低音",
    mid: "中音",
    treble: "高音",
    presence: "临场",
    type_label: "类型:",
    drive: "驱动",
    sag: "下垂",
    cutoff: "截止",
    hold: "保持",
    low_band: "低频段",
    mid_band: "中频段",
    high_band: "高频段",
    low_freq: "低频分频点",
    high_freq: "高频分频点",
    level: "电平",
    crossover: "分频",

    // Filter types
    filter_highpass: "高通",
    filter_lowpass: "低通",

    // Clipper types
    clipper_soft: "柔和削波",
    clipper_medium: "中等削波",
    clipper_hard: "硬削波",
    clipper_asymmetric: "非对称削波",
    clipper_class_a: "A类电子管前级",

    // Power amp types
    poweramp_class_a: "A类",
    poweramp_class_ab: "AB类",
    poweramp_class_b: "B类",

    // Tone stack models
    tonestack_modern: "现代",
    tonestack_british: "英式",
    tonestack_american: "美式",
    tonestack_flat: "平直",

    // Misc UI labels
    output: "输出:",
    samples: "采样",
    requested: "请求:",
    hz: "赫兹",
    db: "分贝",
    ms: "毫秒",
};
