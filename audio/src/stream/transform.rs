use crate::dsp::{FFTSequence, FoldedFFT};
use crate::stream::buffer::Period;
use crate::stream::SampleRate;
use crate::Instant;

#[derive(Clone, Debug)]
pub struct FFTResult {
    pub end_time: Instant,
    pub width: usize,
    pub sample_rate: SampleRate,
    pub ffts: Vec<FoldedFFT>,
}

pub struct FFT {
    width: usize,
    fft: FFTSequence,
}

impl FFT {
    pub fn new(width: usize) -> FFT {
        FFT {
            width,
            fft: FFTSequence::new(width),
        }
    }

    pub fn transform(&self, period: &Period) -> FFTResult {
        assert!(self.width == period.len());
        let mut res = FFTResult {
            end_time: period.end_time(),
            width: self.width,
            sample_rate: period.sample_rate(),
            ffts: Vec::new(),
        };
        for ch in period.channels() {
            res.ffts.push(self.fft.fft(&ch).into_polar().into_folded())
        }
        res
    }
}
