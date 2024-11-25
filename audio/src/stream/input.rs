use super::executor::CHANNEL_MAX;
use async_channel;
use async_channel::{Receiver, TryRecvError, TrySendError};
use cpal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::marker::PhantomData;

use super::pipeline::Step;

// TODO: move other users to use the new location of these:
pub use super::{ChannelCount, Frame, Instant, SampleRate};

#[derive(Debug)]
pub enum InputError {
    DeviceClosed,
    StreamEnded,
}

pub trait Input<'a> {
    type Item;
    fn read(&'a mut self) -> Result<Self::Item, InputError>;
    fn try_read(&'a mut self) -> Result<Option<Self::Item>, InputError>;
}

impl<'a, T, I: Iterator<Item = T>> Input<'a> for I {
    type Item = T;

    fn try_read(&mut self) -> Result<Option<T>, InputError> {
        Ok(self.next())
    }

    fn read(&mut self) -> Result<T, InputError> {
        self.next().ok_or(InputError::StreamEnded)
    }
}

pub struct InputAdapter<'a, I: Input<'a>, S: Step<'a, Input = I::Item>> {
    input: I,
    step: S,
    next_outputs: Option<<<S as Step<'a>>::Result as IntoIterator>::IntoIter>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, I, S> InputAdapter<'a, I, S>
where
    I: Input<'a>,
    S: Step<'a, Input = I::Item>,
{
    pub fn new(input: I, step: S) -> InputAdapter<'a, I, S> {
        InputAdapter {
            input,
            step,
            next_outputs: None,
            _phantom: PhantomData,
        }
    }

    fn maybe_read_once(&'a mut self) -> Option<Result<S::Output, InputError>> {
        if let Some(output) = self.next_outputs.as_mut() {
            if let Some(output) = output.next() {
                return Some(Ok(output));
            }
        }
        let next_input = match self.input.read() {
            Ok(input) => input,
            Err(e) => return Some(Err(e)),
        };
        self.next_outputs = Some(self.step.process(next_input).into_iter());
        None
    }
}

impl<'a, I, S> Input<'a> for InputAdapter<'a, I, S>
where
    I: Input<'a>,
    S: Step<'a, Input = I::Item>,
{
    type Item = S::Output;

    fn read(&'a mut self) -> Result<Self::Item, InputError> {
        // First, check if we already have the next output ready
        if let Some(output) = self.next_outputs.as_mut() {
            if let Some(output) = output.next() {
                return Ok(output);
            }
        }

        // Read inputs and process them with the adapter step until an output
        // item is produced.
        todo!()
        // let input = &mut self.input;
        // let step = &mut self.step;
        // loop {
        // self.input.read();
        // let next_input = input.read()?;
        // let mut results = step.process(next_input).into_iter();
        // if let Some(result) = results.next() {
        //     self.next_outputs = Some(results);
        //     return Ok(result);
        // }
        // }
    }

    fn try_read(&mut self) -> Result<Option<Self::Item>, InputError> {
        todo!()
        // // First, check if we already have the next output ready
        // if let Some(outputs) = self.next_outputs {
        //     if let Some(output) = outputs.next() {
        //         return Ok(Some(output));
        //     }
        // }

        // // Read inputs and process them with the adapter step until either an
        // // output item is produced, or no input is currently available.
        // loop {
        //     if let Some(next_input) = self.input.try_read()? {
        //         let results = self.step.process(next_input).into_iter();
        //         if let Some(result) = results.next() {
        //             self.next_outputs = Some(results);
        //             return Ok(Some(result));
        //         }
        //     } else {
        //         return Ok(None);
        //     }
        // }
    }
}

/// Opens a stream from an audio input device, receives sample data callbacks
/// (which are called by a thread owned by the audio library), and sends the
/// data to consuming threads via `async_channel`.
pub struct InputDevice {
    pub frames: Receiver<Frame>,
    // This owns the input callbacks (and will close the stream when dropped).
    _stream: Box<dyn StreamTrait>,
}

impl InputDevice {
    pub fn new(channels: ChannelCount, sample_rate: SampleRate) -> InputDevice {
        let host = cpal::default_host();
        // TODO: some way of selecting from available devices?
        let device = host.default_input_device().unwrap();

        // Find supported config for the desired number of channels:
        let mut supported: Option<cpal::SupportedStreamConfigRange> = None;
        for c in device.supported_input_configs().unwrap() {
            if c.channels() == channels.0 {
                supported = Some(c);
                break;
            }
        }

        // TODO: what if the desired channel count isn't supported?!
        // TODO: make sure the desired sample rate is actually supported and
        // figure out how to handle it not being so.
        // TODO: set a buffer size (within supported range) instead of just
        // using the default? cpal has a warning that some devices default
        // to very large buffers resulting in high input latency.
        let config = supported
            .unwrap()
            .with_sample_rate(cpal::SampleRate(sample_rate.0));

        let (sender, receiver) = async_channel::bounded(CHANNEL_MAX);
        let stream = Box::new(
            device
                .build_input_stream(
                    &config.config(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        match sender.try_send(Frame {
                            channels,
                            sample_rate,
                            samples: Vec::from(data),
                        }) {
                            Err(TrySendError::Full(_)) => {
                                // TODO: does this need any more handling?
                                // The consumer could notice a drop by watching
                                // the frame number.
                                println!("InputDevice: dropped {} samples", data.len());
                            }
                            Err(TrySendError::Closed(_)) => {
                                // TODO: close the stream?
                                println!("No receiver for {} samples", data.len());
                            }
                            Ok(()) => {}
                        }
                    },
                    move |_err| {
                        // Can these be transient?
                        todo!();
                    },
                    None, // blocking
                )
                .unwrap(),
        );
        // Apparently *some* platforms don't automatically start the stream
        // so this is possibly necessary.
        stream.play().expect("Failed to start stream");

        InputDevice {
            frames: receiver,
            _stream: stream,
        }
    }
}

impl Input<'_> for InputDevice {
    type Item = Frame;

    fn read(&mut self) -> Result<Frame, InputError> {
        match self.frames.recv_blocking() {
            Ok(f) => Ok(f),
            Err(_) => Err(InputError::DeviceClosed),
        }
    }

    fn try_read(&mut self) -> Result<Option<Frame>, InputError> {
        match self.frames.try_recv() {
            Ok(f) => Ok(Some(f)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Closed) => Err(InputError::DeviceClosed),
        }
    }
}
