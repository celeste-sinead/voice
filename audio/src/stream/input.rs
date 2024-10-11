use super::executor::CHANNEL_MAX;
use async_channel;
use async_channel::{Receiver, TryRecvError, TrySendError};
use cpal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[derive(PartialEq, Eq, Copy, Clone)]
pub struct ChannelCount(u16);

impl ChannelCount {
    pub fn new(c: u16) -> ChannelCount {
        ChannelCount(c)
    }
}

impl From<ChannelCount> for u16 {
    fn from(v: ChannelCount) -> u16 {
        v.0
    }
}

impl From<ChannelCount> for usize {
    fn from(v: ChannelCount) -> usize {
        v.0 as usize
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SampleRate(u32);

impl SampleRate {
    pub fn new(s: u32) -> SampleRate {
        SampleRate(s)
    }
}

impl From<SampleRate> for u32 {
    fn from(v: SampleRate) -> u32 {
        v.0
    }
}

impl From<SampleRate> for usize {
    fn from(v: SampleRate) -> usize {
        v.0 as usize
    }
}

impl From<SampleRate> for f32 {
    fn from(v: SampleRate) -> f32 {
        v.0 as f32
    }
}

/// Represents a point in time, in seconds, in a signal
/// Essentially the same as std::time::Instant, but the latter is unusably
/// opaque.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instant(f32);

impl Instant {
    pub fn from_sample_num(sample: usize, rate: SampleRate) -> Instant {
        Instant(sample as f32 / f32::from(rate))
    }
}

impl From<Instant> for f32 {
    fn from(v: Instant) -> f32 {
        v.0 as f32
    }
}

/// A batch of samples received from an input device.
/// If multi-channel, these will be interlaced (I think lol)
pub struct Frame {
    pub channels: ChannelCount,
    pub sample_rate: SampleRate,
    pub samples: Vec<f32>,
}

#[derive(Debug)]
pub enum InputError {
    DeviceClosed,
    StreamEnded,
}

pub trait Input {
    fn next(&mut self) -> Result<Frame, InputError>;
    fn try_next(&mut self) -> Result<Option<Frame>, InputError>;
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
        // TODO: these should be parameters. If they were generic constants
        // the type system could enforce that later processing steps match.
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

impl Input for InputDevice {
    fn next(&mut self) -> Result<Frame, InputError> {
        match self.frames.recv_blocking() {
            Ok(f) => Ok(f),
            Err(_) => Err(InputError::DeviceClosed),
        }
    }

    fn try_next(&mut self) -> Result<Option<Frame>, InputError> {
        match self.frames.try_recv() {
            Ok(f) => Ok(Some(f)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Closed) => Err(InputError::DeviceClosed),
        }
    }
}

/// Implements Input for an iterator that returns samples for a single channel
/// (i.e. accumulates those results into `Frame`s).
/// Assumes that once the iterator is exhausted, it will never return more
/// results.
pub struct IteratorInput {
    iter: Box<dyn Iterator<Item = f32>>,
    sample_rate: SampleRate,
    frame_len: usize,
}

impl IteratorInput {
    // Smallish for tests that want to use small buffers; it probably doesn't
    // really matter what this is set to most of the time
    pub const DEFAULT_FRAME_LEN: usize = 16;

    pub fn new(
        iter: Box<dyn Iterator<Item = f32>>,
        sample_rate: SampleRate,
        frame_len: usize,
    ) -> IteratorInput {
        IteratorInput {
            iter,
            sample_rate,
            frame_len,
        }
    }

    pub fn with_frame_len(mut self, new_len: usize) -> Self {
        self.frame_len = new_len;
        self
    }
}

impl Input for IteratorInput {
    fn try_next(&mut self) -> Result<Option<Frame>, InputError> {
        let mut res = Frame {
            channels: ChannelCount::new(1),
            sample_rate: self.sample_rate,
            samples: Vec::new(),
        };
        for _ in 0..self.frame_len {
            match self.iter.next() {
                Some(s) => res.samples.push(s),
                None => return Ok(None),
            }
        }
        Ok(Some(res))
    }
    fn next(&mut self) -> Result<Frame, InputError> {
        match self.try_next() {
            Ok(Some(f)) => Ok(f),
            Ok(None) => Err(InputError::StreamEnded),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_input() {
        let mut input = IteratorInput::new(
            Box::new((0..8).map(|i| i as f32)),
            SampleRate::new(44100),
            4,
        );
        let f = input.next().unwrap();
        assert_eq!(f.samples, [0., 1., 2., 3.]);
        let f = input.next().unwrap();
        assert_eq!(f.samples, [4., 5., 6., 7.]);
        assert!(input.next().is_err());
    }
}
