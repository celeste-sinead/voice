use std::fs::File;
use std::io::BufWriter;
use std::thread;

use async_channel;
use async_channel::{Receiver, RecvError, Sender};
use hound;

use super::input::{Frame, CHANNEL_MAX, DEFAULT_CHANNELS, DEFAULT_SAMPLE_RATE};

// the fuck is a hound
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
        let writer = hound::WavWriter::create("session.wav", spec).unwrap();
        WavWriter {
            frames_in,
            spec,
            writer,
            unflushed_count: 0,
            flush_every: DEFAULT_SAMPLE_RATE as usize,
            frame_sender,
            frames,
        }
    }

    pub fn write_one(&mut self) -> Result<(), ()> {
        match self.frames_in.recv_blocking() {
            Ok(f) => {
                assert!(f.channels == self.spec.channels);
                assert!(f.sample_rate == self.spec.sample_rate);
                for s in f.samples.iter() {
                    if let Err(e) = self.writer.write_sample(*s) {
                        println!("Failed to write sample: {}", e);
                        return Err(());
                    }
                }

                self.unflushed_count += f.samples.len();
                if self.unflushed_count > self.flush_every {
                    if let Err(e) = self.writer.flush() {
                        println!("Failed to flush samples: {}", e);
                        return Err(());
                    }
                    self.unflushed_count = 0;
                }

                match self.frame_sender.send_blocking(f) {
                    Ok(()) => Ok(()), // is this where the audio sample would be written?
                    Err(_) => Err(()),
                }
            }
            Err(RecvError) => Err(()),
        }
    }

    pub fn run(mut self) -> thread::JoinHandle<()> {
        thread::spawn(move || loop {
            if let Err(_) = self.write_one() {
                break;
            }
        })
    }
}
