#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Amp,
    Effects,
    Cabinet,
    Io,
}
