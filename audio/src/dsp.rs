use std::f32::consts::PI;
use std::iter::zip;
use std::sync::Arc;

use approx::AbsDiffEq;
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

    pub fn fft(&self, period: &ChannelPeriod) -> CartesianFFT {
        let mut res: Vec<Complex<f32>> =
            period.iter().map(|y| Complex { re: *y, im: 0. }).collect();
        self.fft.process(&mut res);
        CartesianFFT(res)
    }
}

/// The result of a FFT, in cartesian form (re + im * i)
#[derive(Clone, Debug, PartialEq)]
pub struct CartesianFFT(pub Vec<Complex<f32>>);

/// The result of a FFT in polar form (r * e ^ (i * Θ))
/// i.e. magnitude + phase which is generally more useful for display
#[derive(Clone, Debug, PartialEq)]
pub struct PolarFFT(pub Vec<(f32, f32)>);

impl CartesianFFT {
    /// Convert to polar form, attempting to unwrap phase (i.e. remove
    /// PI <-> -PI wrap-around discontinuities)
    pub fn to_polar(self) -> PolarFFT {
        PolarFFT(self.0.into_iter().map(|y| y.to_polar()).collect())
    }
}

impl PolarFFT {
    pub fn unwrap_phase(&mut self) {
        let mut prev_wrapped = (0., 0.);
        let mut prev = (0., 0.);
        for cur in &mut self.0 {
            // If the absolute difference betweneen the current and previous
            // (wrapped) phases is > PI, it could be made smaller by adding
            // or subtracting 2*PI, which is our heuristic for wrapping.
            let mut diff = cur.1 - prev_wrapped.1;
            if diff > PI {
                diff -= 2. * PI;
            } else if diff < -PI {
                diff += 2. * PI;
            }
            prev_wrapped = *cur;
            // Apply the wrap-adjusted difference to the previous unwrapped
            // phase to get the current unwrapped phase, in order to preserve
            // the number of rotations that's been accumulated.
            cur.1 = prev.1 + diff;
            prev = *cur;
        }
    }
}

impl AbsDiffEq for PolarFFT {
    type Epsilon = f32;

    fn default_epsilon() -> f32 {
        f32::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &PolarFFT, epsilon: Self::Epsilon) -> bool {
        zip(self.0.iter(), other.0.iter())
            .all(|((r1, p1), (r2, p2))| r1.abs_diff_eq(r2, epsilon) && p1.abs_diff_eq(p2, epsilon))
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

    #[test]
    fn polar_unwrap_positive() {
        let fft = CartesianFFT(vec![
            Complex { re: -1., im: 1. },  // phase = 3/4 * PI
            Complex { re: -1., im: -1. }, // phase wraps to -3/4 * PI
            Complex { re: 1., im: 0. },
            Complex { re: -1., im: 1. },
            Complex { re: -1., im: -1. }, // another wrap
            Complex { re: 1., im: 0. },
            Complex { re: -1., im: 1. },
            Complex { re: -1., im: -1. }, // another wrap
            Complex { re: -1., im: 1. },  // and then unwrap
        ]);
        let mut polar = fft.to_polar();

        polar.unwrap_phase();
        let sq2 = 2f32.sqrt();
        assert_abs_diff_eq!(
            polar,
            PolarFFT(vec![
                (sq2, 0.75 * PI),
                (sq2, 1.25 * PI),
                (1.0, 2.00 * PI),
                (sq2, 2.75 * PI),
                (sq2, 3.25 * PI),
                (1.0, 4.00 * PI),
                (sq2, 4.75 * PI),
                (sq2, 5.25 * PI),
                (sq2, 4.75 * PI),
            ]),
            epsilon = 1e-6
        );

        // unwrap_phase should be idempotent
        let mut polar2 = polar.clone();
        polar2.unwrap_phase();
        assert_eq!(polar, polar2);
    }

    #[test]
    fn polar_unwrap_negative() {
        let fft = CartesianFFT(vec![
            Complex { re: -1., im: -1. }, // phase = -3/4 * PI
            Complex { re: -1., im: 1. },  // phase wraps to 3/4 * PI
            Complex { re: 1., im: 0. },
            Complex { re: -1., im: -1. },
            Complex { re: -1., im: 1. }, // another wrap
            Complex { re: 1., im: 0. },
            Complex { re: -1., im: -1. },
            Complex { re: -1., im: 1. },  // another wrap
            Complex { re: -1., im: -1. }, // and then unwrap
        ]);
        let mut polar = fft.to_polar();

        polar.unwrap_phase();
        let sq2 = 2f32.sqrt();
        assert_abs_diff_eq!(
            polar,
            PolarFFT(vec![
                (sq2, -0.75 * PI),
                (sq2, -1.25 * PI),
                (1.0, -2.00 * PI),
                (sq2, -2.75 * PI),
                (sq2, -3.25 * PI),
                (1.0, -4.00 * PI),
                (sq2, -4.75 * PI),
                (sq2, -5.25 * PI),
                (sq2, -4.75 * PI),
            ]),
            epsilon = 1e-6
        );

        // unwrap_phase should be idempotent
        let mut polar2 = polar.clone();
        polar2.unwrap_phase();
        assert_eq!(polar, polar2);
    }
}
