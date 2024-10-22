use std::f32::consts::PI;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::iter::zip;
use std::sync::Arc;

use approx::AbsDiffEq;
use num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use crate::stream::buffer::ChannelPeriod;
use crate::stream::input::SampleRate;

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
        let mut values: Vec<Complex<f32>> =
            period.iter().map(|y| Complex { re: *y, im: 0. }).collect();
        self.fft.process(&mut values);
        CartesianFFT {
            values,
            sample_rate: period.sample_rate(),
        }
    }
}

/// The result of a FFT, in cartesian form (re + im * i)
#[derive(Clone, Debug, PartialEq)]
pub struct CartesianFFT {
    pub values: Vec<Complex<f32>>,
    pub sample_rate: SampleRate,
}

impl CartesianFFT {
    /// Convert to polar form, attempting to unwrap phase (i.e. remove
    /// PI <-> -PI wrap-around discontinuities)
    pub fn into_polar(self) -> PolarFFT {
        PolarFFT {
            values: self.values.into_iter().map(|y| y.to_polar()).collect(),
            sample_rate: self.sample_rate,
        }
    }

    /// Convenient but inefficient; use FFTSequence to compute many FFTs
    pub fn from_real_signal(signal: Vec<f32>, sample_rate: SampleRate) -> CartesianFFT {
        let mut values: Vec<Complex<f32>> =
            signal.into_iter().map(|y| Complex::new(y, 0.)).collect();
        FftPlanner::new()
            .plan_fft_forward(values.len())
            .process(&mut values);
        CartesianFFT {
            values,
            sample_rate,
        }
    }
}

/// The result of a FFT in polar form (r * e ^ (i * Θ))
/// i.e. magnitude + phase which is generally more useful for display
#[derive(Clone, Debug, PartialEq)]
pub struct PolarFFT {
    pub values: Vec<(f32, f32)>,
    pub sample_rate: SampleRate,
}

impl PolarFFT {
    pub fn unwrap_phase(&mut self) {
        let mut prev_wrapped = (0., 0.);
        let mut prev = (0., 0.);
        for cur in &mut self.values {
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

    pub fn into_folded(self) -> FoldedFFT {
        let n = self.values.len();
        let mut res = FoldedFFT {
            values: self.values,
            sample_rate: self.sample_rate,
            unfolded_length: n,
        };

        // Delete all negative frequency conjugates :3
        res.values.truncate(n / 2 + 1);

        // Apply the 1/N normalization factor from the inverse FFT to
        // magnitudes, making them interpretable as the physical amplitude of
        // that frequency component of teh signal. Multiply values that have
        // a conjugate by 2 to account for the removal of its magnitude.
        let folded_len = res.values.len(); // for the borrow checker
        for (i, y) in res.values.iter_mut().enumerate() {
            if i == 0 {
                // DC never has a conjugate
                y.0 /= n as f32;
            } else if (i == folded_len - 1) && (n % 2 == 0) {
                // If width is odd, the highest positive frequency has no
                // conjugate
                y.0 /= n as f32;
            } else {
                y.0 *= 2. / n as f32;
            }
        }
        res
    }
}

impl AbsDiffEq for PolarFFT {
    type Epsilon = f32;

    fn default_epsilon() -> f32 {
        f32::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &PolarFFT, epsilon: Self::Epsilon) -> bool {
        if self.values.len() != other.values.len() {
            return false;
        }
        zip(self.values.iter(), other.values.iter())
            .all(|((r1, p1), (r2, p2))| r1.abs_diff_eq(r2, epsilon) && p1.abs_diff_eq(p2, epsilon))
    }
}

/// The result of a FFT, with magnitudes normalized, and negative frequencies
/// folded into their positive frequency conjugates (which gives magnitudes and
/// phases ranging from DC to the nyquist frequency, which is generally what
/// you want for physical interpretation)
#[derive(Clone, Debug, PartialEq)]
pub struct FoldedFFT {
    pub values: Vec<(f32, f32)>,
    sample_rate: SampleRate,
    /// This is needed for inversion and Hz computation because we wouldn't
    /// otherwise know if N is (values.len() * 2) or (values.len() * 2 + 1).
    unfolded_length: usize,
}

impl FoldedFFT {
    pub fn frequencies(&self) -> Box<dyn Iterator<Item = Hz> + '_> {
        Box::new(
            (0..self.values.len())
                .map(|i| Hz(i as f32 * f32::from(self.sample_rate) / self.unfolded_length as f32)),
        )
    }

    pub fn nyquist_frequency(&self) -> Hz {
        Hz(f32::from(self.sample_rate) / 2.0)
    }
}

impl AbsDiffEq for FoldedFFT {
    type Epsilon = f32;

    fn default_epsilon() -> f32 {
        f32::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &FoldedFFT, epsilon: Self::Epsilon) -> bool {
        if self.unfolded_length != other.unfolded_length {
            return false;
        }
        assert!(self.values.len() == other.values.len()); // implied by unfolded_length
        zip(self.values.iter(), other.values.iter())
            .all(|((r1, p1), (r2, p2))| r1.abs_diff_eq(r2, epsilon) && p1.abs_diff_eq(p2, epsilon))
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
    fn polar_unwrap_positive() {
        let fft = CartesianFFT {
            values: vec![
                Complex { re: -1., im: 1. },  // phase = 3/4 * PI
                Complex { re: -1., im: -1. }, // phase wraps to -3/4 * PI
                Complex { re: 1., im: 0. },
                Complex { re: -1., im: 1. },
                Complex { re: -1., im: -1. }, // another wrap
                Complex { re: 1., im: 0. },
                Complex { re: -1., im: 1. },
                Complex { re: -1., im: -1. }, // another wrap
                Complex { re: -1., im: 1. },  // and then unwrap
            ],
            sample_rate: SampleRate::new(42),
        };
        let mut polar = fft.into_polar();

        polar.unwrap_phase();
        let sq2 = 2f32.sqrt();
        assert_abs_diff_eq!(
            polar,
            PolarFFT {
                values: vec![
                    (sq2, 0.75 * PI),
                    (sq2, 1.25 * PI),
                    (1.0, 2.00 * PI),
                    (sq2, 2.75 * PI),
                    (sq2, 3.25 * PI),
                    (1.0, 4.00 * PI),
                    (sq2, 4.75 * PI),
                    (sq2, 5.25 * PI),
                    (sq2, 4.75 * PI),
                ],
                sample_rate: SampleRate::new(42)
            },
            epsilon = 1e-6
        );

        // unwrap_phase should be idempotent
        let mut polar2 = polar.clone();
        polar2.unwrap_phase();
        assert_eq!(polar, polar2);
    }

    #[test]
    fn polar_unwrap_negative() {
        let fft = CartesianFFT {
            values: vec![
                Complex { re: -1., im: -1. }, // phase = -3/4 * PI
                Complex { re: -1., im: 1. },  // phase wraps to 3/4 * PI
                Complex { re: 1., im: 0. },
                Complex { re: -1., im: -1. },
                Complex { re: -1., im: 1. }, // another wrap
                Complex { re: 1., im: 0. },
                Complex { re: -1., im: -1. },
                Complex { re: -1., im: 1. },  // another wrap
                Complex { re: -1., im: -1. }, // and then unwrap
            ],
            sample_rate: SampleRate::new(42),
        };
        let mut polar = fft.into_polar();

        polar.unwrap_phase();
        let sq2 = 2f32.sqrt();
        assert_abs_diff_eq!(
            polar,
            PolarFFT {
                values: vec![
                    (sq2, -0.75 * PI),
                    (sq2, -1.25 * PI),
                    (1.0, -2.00 * PI),
                    (sq2, -2.75 * PI),
                    (sq2, -3.25 * PI),
                    (1.0, -4.00 * PI),
                    (sq2, -4.75 * PI),
                    (sq2, -5.25 * PI),
                    (sq2, -4.75 * PI),
                ],
                sample_rate: SampleRate::new(42)
            },
            epsilon = 1e-6
        );

        // unwrap_phase should be idempotent
        let mut polar2 = polar.clone();
        polar2.unwrap_phase();
        assert_eq!(polar, polar2);
    }

    #[test]
    fn fold_even() {
        let fft =
            CartesianFFT::from_real_signal(vec![0., 1., 2., 3.], SampleRate::new(42)).into_polar();
        assert_abs_diff_eq!(
            fft,
            PolarFFT {
                values: vec![(6., 0.), (2.83, 2.36), (2.0, 3.14), (2.83, -2.36)],
                sample_rate: SampleRate::new(42)
            },
            epsilon = 1e-2
        );
        let folded = fft.into_folded();
        assert_abs_diff_eq!(
            folded,
            FoldedFFT {
                values: vec![(1.5, 0.), (2.83 / 2., 2.36), (0.5, 3.14)],
                sample_rate: SampleRate::new(42),
                unfolded_length: 4
            },
            epsilon = 1e-2
        );
    }

    #[test]
    fn fold_odd() {
        let fft = CartesianFFT::from_real_signal(vec![0., 1., 2., 3., 4.], SampleRate::new(42))
            .into_polar();
        assert_abs_diff_eq!(
            fft,
            PolarFFT {
                values: vec![
                    (10., 0.),
                    (4.25, 2.20),
                    (2.63, 2.83),
                    (2.63, -2.83),
                    (4.25, -2.20)
                ],
                sample_rate: SampleRate::new(42)
            },
            epsilon = 1e-2
        );
        let folded = fft.into_folded();
        assert_abs_diff_eq!(
            folded,
            FoldedFFT {
                values: vec![(2., 0.), (4.25 / 2.5, 2.2), (2.63 / 2.5, 2.83)],
                sample_rate: SampleRate::new(42),
                unfolded_length: 5
            },
            epsilon = 1e-2
        );
    }

    #[test]
    fn folded_frequencies() {
        let fft = FoldedFFT {
            values: [(0., 0.); 6].into_iter().collect(),
            sample_rate: SampleRate::new(20),
            unfolded_length: 10,
        };
        assert_eq!(
            fft.frequencies().collect::<Vec<Hz>>(),
            vec![Hz(0.), Hz(2.), Hz(4.), Hz(6.), Hz(8.), Hz(10.)]
        );
    }
}
