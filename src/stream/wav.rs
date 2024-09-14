use std::fs::File;
use std::io::BufWriter;
use std::thread;

use async_channel;
use async_channel::{Receiver, RecvError, Sender};
use hound; // (provides .wav encoding)

use super::executor::CHANNEL_MAX;
use super::input::{Frame, DEFAULT_CHANNELS, DEFAULT_SAMPLE_RATE};

/// Used to write all the samples received from an audio input to a file
/// (currently always ./session.wav), for ad-hoc testing and debugging.
/// This is meant to consume from InputStream::frames, and WavWriter::frames
/// is intended to be consumed by the application (or some additional
/// processing step).
pub struct WavWriter {
    frames_in: Receiver<Frame>,
    spec: hound::WavSpec,
    writer: hound::WavWriter<BufWriter<File>>,
    unflushed_count: usize,
    flush_every: usize,
    frame_sender: Sender<Frame>,
    pub frames: Receiver<Frame>,
}

impl WavWriter {
    pub fn new(frames_in: Receiver<Frame>) -> WavWriter {
        let (frame_sender, frames) = async_channel::bounded(CHANNEL_MAX);
        let spec = hound::WavSpec {
            channels: DEFAULT_CHANNELS,
            sample_rate: DEFAULT_SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        // TODO: parameterize the output file, have an option to not write the
        // file, etc.
        let writer = hound::WavWriter::create("session.wav", spec).unwrap();
        WavWriter {
            frames_in,
            spec,
            writer,
            unflushed_count: 0,
            flush_every: DEFAULT_SAMPLE_RATE as usize, // i.e. every 1 second
            frame_sender,
            frames,
        }
    }

    pub fn write_one(&mut self) -> Result<(), ()> {
        match self.frames_in.recv_blocking() {
            Ok(f) => {
                // TODO: these could be generic constants and then this could
                // be guaranteed by the compiler?
                assert!(u16::from(f.channels) == self.spec.channels);
                assert!(u32::from(f.sample_rate) == self.spec.sample_rate);

                // Add the samples to the write buffer
                for s in f.samples.iter() {
                    if let Err(e) = self.writer.write_sample(*s) {
                        println!("Failed to write sample: {}", e);
                        return Err(());
                    }
                }

                // Periodically flush the file, so it's a valid .wav up to the
                // last ~second in the case of a crash
                self.unflushed_count += f.samples.len();
                if self.unflushed_count > self.flush_every {
                    if let Err(e) = self.writer.flush() {
                        println!("Failed to flush samples: {}", e);
                        return Err(());
                    }
                    self.unflushed_count = 0;
                }

                // Pass the samples on to whatever wants them next
                match self.frame_sender.send_blocking(f) {
                    Ok(()) => Ok(()),
                    // This means whatever next step has closed its end of the
                    // channel; the caller should probably shut down (when this
                    // struct is dropped, the input stream will be passed the
                    // error, and will hopefully also shut down...)
                    Err(_) => Err(()),
                }
            }
            Err(RecvError) => Err(()),
        }
    }

    /// Spawn a thread that receives frames, writes them out, and then passes
    /// them on, until either the input or output channel is closed from the
    /// other end.
    pub fn run(mut self) -> thread::JoinHandle<()> {
        thread::spawn(move || loop {
            if let Err(_) = self.write_one() {
                break;
            }
        })
    }
}
