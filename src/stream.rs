use std::fs::File;
use std::io::BufWriter;
use std::thread;

use async_channel;
use async_channel::{Receiver, RecvError, Sender, TrySendError};
use cpal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound;

// The maximum length of channels passing audio data amongst threads
// This shouldn't be large; if a consumer isn't keeping up long channels are
// just going to add latency to the situation.
const CHANNEL_MAX: usize = 16;

const DEFAULT_CHANNELS: u16 = 1;
const DEFAULT_SAMPLE_RATE: u32 = 44100;

pub struct Frame {
    pub number: usize,
    #[allow(dead_code)]
    pub channels: u16,
    #[allow(dead_code)]
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

pub struct InputStream {
    pub frames: Receiver<Frame>,
    _stream: Box<dyn StreamTrait>,
}

impl InputStream {
    pub fn new() -> InputStream {
        let (tx, rx) = async_channel::bounded(CHANNEL_MAX);

        let channels = DEFAULT_CHANNELS;
        let sample_rate = DEFAULT_SAMPLE_RATE;
        let host = cpal::default_host();
        let device = host.default_input_device().unwrap();
        let mut supported: Option<cpal::SupportedStreamConfigRange> = None;
        for c in device.supported_input_configs().unwrap() {
            if c.channels() == channels {
                supported = Some(c);
                break;
            }
        }
        // In theory, should check this rate is supported:
        let config = supported
            .unwrap()
            .with_sample_rate(cpal::SampleRate(sample_rate));
        let mut frame_count: usize = 0;
        let stream = Box::new(
            device
                .build_input_stream(
                    &config.config(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let len = data.len();
                        match tx.try_send(Frame {
                            number: frame_count,
                            channels,
                            sample_rate,
                            samples: Vec::from(data),
                        }) {
                            Err(TrySendError::Full(_)) => {
                                println!("InputStream: dropped {} samples", len);
                            }
                            Err(TrySendError::Closed(_)) => {
                                println!("No receiver for {} samples", len);
                            }
                            Ok(()) => {}
                        }
                        frame_count += 1;
                    },
                    move |err| {
                        println!("Stream error: {}", err);
                    },
                    None, // blocking
                )
                .unwrap(),
        );

        InputStream {
            frames: rx,
            _stream: stream,
        }
    }
}

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
                    Ok(()) => Ok(()),
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
