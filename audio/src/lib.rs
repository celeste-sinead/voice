use std::time::Duration;

#[cfg(test)]
#[macro_use]
extern crate approx;

pub mod dsp;
pub mod stream;
pub mod synth;

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
