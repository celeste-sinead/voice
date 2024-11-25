use std::f32::consts::PI;

use crate::dsp::Decibels;
use crate::stream::input::SampleRate;
use crate::stream::pipeline::Step;

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

    pub fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency
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

pub struct Gain {
    gain: f32,
}

impl Gain {
    pub fn new(gain: Decibels) -> Gain {
        Gain {
            // sqrt converts from power ratio to amplitude ratio
            gain: gain.into_full_scale().sqrt(),
        }
    }

    pub fn set_gain(&mut self, gain: Decibels) {
        self.gain = gain.into_full_scale().sqrt();
    }
}

impl Default for Gain {
    fn default() -> Gain {
        Gain::new(Decibels::new(0.))
    }
}

impl<'a> Step<'a> for Gain {
    type Input = f32;
    type Output = f32;
    type Result = Option<f32>;

    fn process(&mut self, v: f32) -> Option<f32> {
        Some(v * self.gain)
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
