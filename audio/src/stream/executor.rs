use std::thread;

use async_channel::{Receiver, Sender, TryRecvError};

use super::buffer::{PeriodBuffer, SampleBuffer};
use super::input::{Input, InputDevice};
use super::output::OutputDevice;
use super::pipeline::{Pipeline, Step};
use super::transform::FFT;
use super::wav::WavWriter;
use super::{ChannelCount, Frame, SampleRate};
use crate::dsp::Decibels;
use crate::{dsp, Message, RMSLevels};

// The maximum length of channels passing audio data amongst threads
// This shouldn't be large; if a consumer isn't keeping up long channels are
// just going to add latency to the situation.
pub const CHANNEL_MAX: usize = 16;

pub struct Executor {
    channels: ChannelCount,
    sample_rate: SampleRate,
    writer: WavWriter,
    periods: PeriodBuffer,
    fft: FFT,
    sender: Sender<Message>,
}

/// Encapsulates the audio processing thread, which waits for samples from the
/// input device, computes the results we want, and sends those results to
/// the UI thread for display.
impl Executor {
    /// Create an executor that will send display updates via the given Sender
    pub fn new(
        sender: Sender<Message>,
        channels: ChannelCount,
        sample_rate: SampleRate,
    ) -> Executor {
        Executor {
            channels,
            sample_rate,
            writer: WavWriter::new(channels, sample_rate),
            periods: PeriodBuffer::new(
                SampleBuffer::new(channels, sample_rate, usize::from(sample_rate) * 2),
                8192,
                8192,
            ),
            fft: FFT::new(8192),
            sender,
        }
    }

    /// Handle a single frame of samples received from the input device
    fn process(&mut self, frame: &Frame) -> Vec<Message> {
        let mut res = Vec::new();
        self.writer.push(frame).expect("session.wav write error");
        self.periods.push(frame);
        while let Some(p) = self.periods.next() {
            res.push(Message::FFTResult(self.fft.transform(&p)));
            res.push(Message::RMSLevels(RMSLevels {
                time: p.start_time(),
                values: p.channels().into_iter().map(|c| dsp::rms(&c)).collect(),
            }));
        }
        res
    }

    /// The main loop of the audio processing thread
    fn run<T: Input<Item = Frame>>(mut self, mut input: T) {
        loop {
            match input.next() {
                Ok(f) => {
                    for m in self.process(&f) {
                        if let Err(_) = self.sender.send_blocking(m) {
                            println!("Executor exit: UI closed.");
                            return;
                        }
                    }
                }
                Err(_) => {
                    println!("Executor exit: audio input closed.");
                    let _e = self.sender.send_blocking(Message::AudioStreamClosed);
                    return;
                }
            }
        }
    }

    /// Spawn a new thread to run this executor
    pub fn start(self) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            // cpal::StreamTrait isn't Send, so the input device needs to
            // be opened on the executor thread.
            let input = InputDevice::new(self.channels, self.sample_rate);
            self.run(input);
        })
    }
}

#[derive(Debug)]
pub enum Request {
    SetGain(Decibels),
}

#[derive(Debug)]
pub struct Response();

/// TODO: this should be merged with Executor
pub struct PipelineExecutor<I: Input, S: Step<Input = I::Item, Output = Frame>> {
    pipeline: Pipeline<I, S, OutputDevice>,
    receiver: Receiver<Request>,
}

impl<I: Input + Send + 'static, S: Step<Input = I::Item, Output = Frame> + Send + 'static>
    PipelineExecutor<I, S>
{
    pub fn new(
        channels: ChannelCount,
        sample_rate: SampleRate,
        input: I,
        step: S,
    ) -> (Sender<Request>, Receiver<Response>, thread::JoinHandle<()>) {
        let (req_send, req_recv) = async_channel::bounded(CHANNEL_MAX);
        let (_msg_send, msg_recv) = async_channel::bounded(CHANNEL_MAX);
        (
            req_send,
            msg_recv,
            thread::spawn(move || {
                let mut executor = PipelineExecutor {
                    pipeline: Pipeline::new(
                        input,
                        step,
                        OutputDevice::new(channels, sample_rate).unwrap(),
                    ),
                    receiver: req_recv,
                };
                executor.run();
            }),
        )
    }

    fn run(&mut self) {
        loop {
            match self.receiver.try_recv() {
                Ok(req) => println!("Got Request: {:?}", req),
                Err(TryRecvError::Empty) => (),
                Err(_) => todo!(),
            }
            self.pipeline.process_once().unwrap();
        }
    }
}
