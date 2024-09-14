use std::fs::File;
use std::io::BufWriter;

use hound; // (provides .wav encoding)
pub use hound::Result;

use super::input::{ChannelCount, Frame, SampleRate};

/// Used to write all the samples received from an audio input to a file
/// (currently always ./session.wav), for ad-hoc testing and debugging.
/// This is meant to consume from InputStream::frames, and WavWriter::frames
/// is intended to be consumed by the application (or some additional
/// processing step).
pub struct WavWriter {
    spec: hound::WavSpec,
    writer: hound::WavWriter<BufWriter<File>>,
    unflushed_count: usize,
    flush_every: usize,
}

impl WavWriter {
    pub fn new(channels: ChannelCount, sample_rate: SampleRate) -> WavWriter {
        let spec = hound::WavSpec {
            channels: u16::from(channels),
            sample_rate: u32::from(sample_rate),
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        // TODO: parameterize the output file, have an option to not write the
        // file, etc.
        let writer = hound::WavWriter::create("session.wav", spec).unwrap();
        WavWriter {
            spec,
            writer,
            unflushed_count: 0,
            flush_every: usize::from(sample_rate), // i.e. every 1 second
        }
    }

    pub fn push(&mut self, frame: &Frame) -> Result<()> {
        assert!(u16::from(frame.channels) == self.spec.channels);
        assert!(u32::from(frame.sample_rate) == self.spec.sample_rate);

        // Add the samples to the write buffer
        for s in frame.samples.iter() {
            self.writer.write_sample(*s)?;
        }

        // Periodically flush the file, so it's a valid .wav up to the
        // last ~second in the case of a crash
        self.unflushed_count += frame.samples.len();
        if self.unflushed_count > self.flush_every {
            self.writer.flush()?;
            self.unflushed_count = 0;
        }

        Ok(())
    }
}
