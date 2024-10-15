#[cfg(test)]
#[macro_use]
extern crate approx;

pub mod dsp;
pub mod stream;
pub mod synth;

use stream::input::Instant;
pub use stream::transform::FFTResult;

#[derive(Clone, Debug)]
pub struct RMSLevels {
    /// The end time of the measurement period
    pub time: Instant,
    /// Full scale RMS, for each channel
    pub values: Vec<f32>,
}

// The message type that is used to update iced application state
#[derive(Debug, Clone)]
pub enum Message {
    AudioStreamClosed,
    FFTResult(FFTResult),
    RMSLevels(RMSLevels),
}
