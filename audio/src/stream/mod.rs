use std::ops::{Add, Sub};
use std::time::Duration;

use cpal;

pub mod buffer;
pub mod executor;
pub mod input;
pub mod output;
pub mod pipeline;
pub mod transform;
pub mod wav;

#[derive(PartialEq, Eq, Copy, Clone)]
pub struct ChannelCount(u16);

impl ChannelCount {
    pub fn new(c: u16) -> ChannelCount {
        ChannelCount(c)
    }
}

impl From<ChannelCount> for u16 {
    fn from(v: ChannelCount) -> u16 {
        v.0
    }
}

impl From<ChannelCount> for usize {
    fn from(v: ChannelCount) -> usize {
        v.0 as usize
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SampleRate(u32);

impl SampleRate {
    pub fn new(s: u32) -> SampleRate {
        SampleRate(s)
    }
}

impl From<SampleRate> for u32 {
    fn from(v: SampleRate) -> u32 {
        v.0
    }
}

impl From<SampleRate> for usize {
    fn from(v: SampleRate) -> usize {
        v.0 as usize
    }
}

impl From<SampleRate> for f32 {
    fn from(v: SampleRate) -> f32 {
        v.0 as f32
    }
}

impl From<SampleRate> for cpal::SampleRate {
    fn from(v: SampleRate) -> cpal::SampleRate {
        cpal::SampleRate(v.0)
    }
}

/// Represents a point in time, in seconds, in a signal
/// Essentially the same as std::time::Instant, but the latter is unusably
/// opaque.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instant(f32);

impl Instant {
    pub const ZERO: Instant = Instant(0.);

    pub fn from_sample_num(sample: usize, rate: SampleRate) -> Instant {
        Instant(sample as f32 / f32::from(rate))
    }
}

impl Default for Instant {
    fn default() -> Instant {
        Instant(0.)
    }
}

impl From<Instant> for f32 {
    fn from(v: Instant) -> f32 {
        v.0 as f32
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    fn add(self, rhs: Duration) -> Instant {
        Instant(self.0 + rhs.as_secs_f32())
    }
}

impl Sub for Instant {
    type Output = Duration;

    fn sub(self, rhs: Instant) -> Duration {
        Duration::from_secs_f32(self.0 - rhs.0)
    }
}

impl Sub<Duration> for Instant {
    type Output = Instant;

    fn sub(self, rhs: Duration) -> Instant {
        Instant(self.0 - rhs.as_secs_f32())
    }
}

/// A batch of samples received from an input device.
pub struct Frame {
    pub channels: ChannelCount,
    pub sample_rate: SampleRate,
    pub samples: Vec<f32>,
}
