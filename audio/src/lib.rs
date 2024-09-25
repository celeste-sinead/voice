use std::time::Duration;

pub mod dsp;
pub mod stream;

#[derive(Clone, Debug)]
pub struct RMSLevels {
    /// The end time of the measurement period
    pub time: Duration,
    /// Full scale RMS, for each channel
    pub values: Vec<f32>,
}

// The message type that is used to update iced application state
#[derive(Debug, Clone)]
pub enum Message {
    RMSLevels(RMSLevels),
    AudioStreamClosed,
}
