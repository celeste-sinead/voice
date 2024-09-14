use super::executor::CHANNEL_MAX;
use async_channel;
use async_channel::{Receiver, TrySendError};
use cpal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub const DEFAULT_CHANNELS: u16 = 1;
pub const DEFAULT_SAMPLE_RATE: u32 = 44100;

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

#[derive(PartialEq, Eq, Copy, Clone)]
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

/// A batch of samples received from an input device.
/// If multi-channel, these will be interlaced (I think lol)
pub struct Frame {
    #[allow(dead_code)]
    pub number: usize,
    pub channels: ChannelCount,
    pub sample_rate: SampleRate,
    pub samples: Vec<f32>,
}

/// Opens a stream from an audio input device, receives sample data callbacks
/// (which are called by a thread owned by the audio library), and sends the
/// data to consuming threads via `async_channel`.
pub struct InputStream {
    pub frames: Receiver<Frame>,
    // This owns the input callbacks (and will close the stream when dropped).
    _stream: Box<dyn StreamTrait>,
}

impl InputStream {
    pub fn new() -> InputStream {
        // TODO: these should be parameters. If they were generic constants
        // the type system could enforce that later processing steps match.
        let channels = ChannelCount(DEFAULT_CHANNELS);
        let sample_rate = SampleRate(DEFAULT_SAMPLE_RATE);

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

        // Frame counter, to be captured by the callback closure
        let mut frame_count: usize = 0;

        let (sender, receiver) = async_channel::bounded(CHANNEL_MAX);
        let stream = Box::new(
            device
                .build_input_stream(
                    &config.config(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        match sender.try_send(Frame {
                            number: frame_count,
                            channels,
                            sample_rate,
                            samples: Vec::from(data),
                        }) {
                            Err(TrySendError::Full(_)) => {
                                // TODO: does this need any more handling?
                                // The consumer could notice a drop by watching
                                // the frame number.
                                println!("InputStream: dropped {} samples", data.len());
                            }
                            Err(TrySendError::Closed(_)) => {
                                // TODO: close the stream?
                                println!("No receiver for {} samples", data.len());
                            }
                            Ok(()) => {}
                        }
                        frame_count += 1;
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

        println!("Receiver is_closed: {}", receiver.is_closed());
        InputStream {
            frames: receiver,
            _stream: stream,
        }
    }
}
