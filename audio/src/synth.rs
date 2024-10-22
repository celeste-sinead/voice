use std::f32::consts::PI;

use crate::stream::input::SampleRate;

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

/// An Iterator that produces an infinite sinusoid
pub struct SinIterator {
    frequency: f32,
    phase: f32,
    clock: SampleClock,
}

impl SinIterator {
    /// frequency is in Hz, phase is in radians
    pub fn new(sample_rate: SampleRate, frequency: f32, phase: f32) -> SinIterator {
        SinIterator {
            frequency,
            phase,
            clock: SampleClock::new(sample_rate),
        }
    }
}

impl Iterator for SinIterator {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        match self.clock.next() {
            Some(t) => Some((2. * PI * self.frequency * t + self.phase).sin()),
            None => panic!("impossible, clock is infinite"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let samples: Vec<f32> = SinIterator::new(SampleRate::new(4), 1., 0.)
            .zip(0..5)
            .map(|(y, _)| y)
            .collect();
        assert_samples_eq(&samples, &vec![0., 1., 0., -1., 0.])
    }

    #[test]
    fn test_sin_freq_phase() {
        let samples: Vec<f32> = SinIterator::new(SampleRate::new(32), 4., PI / 2.)
            .zip(0..4)
            .map(|(y, _)| y)
            .collect();
        let inv_sqrt_2 = 1.0 / 2f32.sqrt();
        assert_samples_eq(&samples, &vec![1., inv_sqrt_2, 0., -inv_sqrt_2])
    }
}
