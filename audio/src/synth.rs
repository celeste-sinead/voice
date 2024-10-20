use std::f32::consts::PI;

use crate::stream::input::{IteratorInput, SampleRate};

/// An iterator that returns and infinite sequence of sample times (seconds)
/// for a given sample rate (which is a useful base for synthesizing signals)
struct SampleClock {
    i: f32,
    sample_rate: f32,
}

impl SampleClock {
    fn new(sample_rate: SampleRate) -> SampleClock {
        SampleClock {
            i: 0.,
            sample_rate: usize::from(sample_rate) as f32,
        }
    }
}

impl Iterator for SampleClock {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let res = Some(self.i / self.sample_rate);
        self.i += 1.;
        res
    }
}

pub fn sin_iter(
    sample_rate: SampleRate,
    frequency: f32,
    phase: f32,
) -> Box<dyn Iterator<Item = f32>> {
    Box::new(SampleClock::new(sample_rate).map(move |t| (2. * PI * frequency * t + phase).sin()))
}

/// Return an Input that produces an infinite sinusoid
/// frequency is in Hz, phase is in radians
pub fn sin(sample_rate: SampleRate, frequency: f32, phase: f32) -> IteratorInput {
    IteratorInput::new(
        sin_iter(sample_rate, frequency, phase),
        sample_rate,
        IteratorInput::DEFAULT_FRAME_LEN,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::input::Input;

    fn assert_samples_eq(left: &Vec<f32>, right: &Vec<f32>) {
        let eq = if left.len() == right.len() {
            left.iter()
                .zip(right.iter())
                .all(|(l, r)| (l - r).abs() < 1e-6)
        } else {
            false
        };
        if !eq {
            // Reuse the nice error printing
            assert_eq!(left, right);
        }
    }

    #[test]
    fn test_sin() {
        let mut sin = sin(SampleRate::new(4), 1., 0.).with_frame_len(5);
        let f = sin.next().unwrap();
        assert_samples_eq(&f.samples, &vec![0., 1., 0., -1., 0.])
    }

    #[test]
    fn test_sin_freq_phase() {
        let mut sin = sin(SampleRate::new(32), 4., PI / 2.).with_frame_len(4);
        let f = sin.next().unwrap();
        let inv_sqrt_2 = 1.0 / 2f32.sqrt();
        assert_samples_eq(&f.samples, &vec![1., inv_sqrt_2, 0., -inv_sqrt_2])
    }
}
