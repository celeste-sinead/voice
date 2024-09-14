use std::thread;

use async_channel::{Receiver, Sender};

use super::buffer::{InputBuffer, PeriodStream};
use super::input::{
    ChannelCount, Frame, InputStream, SampleRate, DEFAULT_CHANNELS, DEFAULT_SAMPLE_RATE,
};
use super::wav::WavWriter;
use crate::Message;

// The maximum length of channels passing audio data amongst threads
// This shouldn't be large; if a consumer isn't keeping up long channels are
// just going to add latency to the situation.
pub const CHANNEL_MAX: usize = 16;

const DEFAULT_BUFFER_LEN: usize = 2 * DEFAULT_SAMPLE_RATE as usize;

pub struct Executor {
    // writer: WavWriter,
    periods: PeriodStream,
    sender: Sender<Message>,
}

impl Executor {
    pub fn new(sender: Sender<Message>) -> Executor {
        Executor {
            periods: PeriodStream::new(
                InputBuffer::new(
                    ChannelCount::new(DEFAULT_CHANNELS),
                    SampleRate::new(DEFAULT_SAMPLE_RATE),
                    DEFAULT_BUFFER_LEN,
                ),
                DEFAULT_SAMPLE_RATE as usize / 10,
                DEFAULT_SAMPLE_RATE as usize / 10,
            ),
            sender,
        }
    }

    fn run(mut self, frames: Receiver<Frame>) {
        loop {
            match frames.recv_blocking() {
                Ok(f) => {
                    self.periods.push(&f);
                    while let Some(p) = self.periods.next() {
                        if let Err(_) = self.sender.send_blocking(Message::AudioFrame(
                            p.start_sample_num() / 4410,
                            p.len(),
                        )) {
                            println!("Executor exit: UI closed.");
                            return;
                        }
                    }
                }
                Err(_) => {
                    println!("Executor exit: audio input closed.");
                    return;
                }
            }
        }
    }

    pub fn start(self) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            // cpal::StreamTrait isn't Send, so the input device needs to
            // be opened on the executor thread.
            let input = InputStream::new();
            self.run(input.frames);
        })
    }
}
