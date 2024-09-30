use std::sync::Arc;

use num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use crate::stream::buffer::ChannelPeriod;

pub fn rms(period: &ChannelPeriod) -> f32 {
    let sum_sq = period.iter().fold(0.0, |acc, x| acc + x * x);
    let mean_sq = sum_sq / period.len() as f32;
    mean_sq.sqrt()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decibels(f32);

impl Decibels {
    pub fn from_full_scale(fs: f32) -> Decibels {
        Decibels(10. * fs.log10())
    }
}

impl From<Decibels> for f32 {
    fn from(db: Decibels) -> f32 {
        db.0
    }
}

pub struct FFTSequence {
    fft: Arc<dyn Fft<f32>>,
}

impl FFTSequence {
    pub fn new(period_len: usize) -> FFTSequence {
        FFTSequence {
            // nb: reusing the planner is recommended if a lot of these are
            // going to get constructed.
            fft: FftPlanner::new().plan_fft_forward(period_len),
        }
    }

    pub fn fft(&self, period: &ChannelPeriod) -> Vec<Complex<f32>> {
        let mut res: Vec<Complex<f32>> =
            period.iter().map(|y| Complex { re: *y, im: 0. }).collect();
        self.fft.process(&mut res);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::stream::buffer::BufferedInput;
    use crate::stream::input::SampleRate;
    use crate::synth;

    #[test]
    fn test_rms() {
        assert_relative_eq!(
            rms(
                &BufferedInput::new(synth::sin(SampleRate::new(100), 1., 0.), 100)
                    .unwrap()
                    .next()
                    .unwrap()
                    .get_channel(0)
            ),
            1.0 / 2f32.sqrt()
        )
    }

    #[test]
    fn db_from_full_scale() {
        assert_eq!(Decibels::from_full_scale(0.1), Decibels(-10.))
    }
}
