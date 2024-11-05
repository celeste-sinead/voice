use std::fmt;
use std::fmt::{Display, Formatter};

use crate::stream::buffer::ChannelPeriod;

pub mod fft;
pub mod filter;

pub fn rms(period: &ChannelPeriod) -> f32 {
    let sum_sq = period.iter().fold(0.0, |acc, x| acc + x * x);
    let mean_sq = sum_sq / period.len() as f32;
    mean_sq.sqrt()
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Decibels(f32);

impl Decibels {
    pub fn new(db: f32) -> Decibels {
        Decibels(db)
    }

    pub fn from_full_scale(fs: f32) -> Decibels {
        Decibels(10. * fs.log10())
    }

    pub fn into_full_scale(self) -> f32 {
        10f32.powf(self.0 / 10.)
    }
}

impl Display for Decibels {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        self.0.fmt(f)?;
        f.write_str("dB")
    }
}

impl From<Decibels> for f32 {
    fn from(db: Decibels) -> f32 {
        db.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Hz(pub f32);

impl From<Hz> for f32 {
    fn from(v: Hz) -> f32 {
        v.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::stream::buffer::BufferedInput;
    use crate::stream::input::SampleRate;
    use crate::stream::ChannelCount;
    use crate::synth;

    #[test]
    fn test_rms() {
        assert_relative_eq!(
            rms(&BufferedInput::from_sample_input(
                synth::SinIterator::new(SampleRate::new(100), 1., 0.),
                ChannelCount::new(1),
                SampleRate::new(100),
                100
            )
            .unwrap()
            .next()
            .unwrap()
            .get_channel(0)),
            1.0 / 2f32.sqrt()
        )
    }

    #[test]
    fn db_from_full_scale() {
        assert_eq!(Decibels::from_full_scale(0.1), Decibels(-10.))
    }

    #[test]
    fn full_scale_from_db() {
        assert_eq!(Decibels::new(-10.).into_full_scale(), 0.1);
    }
}
