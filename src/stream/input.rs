use async_channel;
use async_channel::{Receiver, TrySendError};
use cpal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

// The maximum length of channels passing audio data amongst threads
// This shouldn't be large; if a consumer isn't keeping up long channels are
// just going to add latency to the situation.
pub const CHANNEL_MAX: usize = 16;

pub const DEFAULT_CHANNELS: u16 = 1;
pub const DEFAULT_SAMPLE_RATE: u32 = 44100;

/// A batch of samples received from an input device.
/// If multi-channel, these will be interlaced (I think lol)
pub struct Frame {
    pub number: usize,
    #[allow(dead_code)]
    pub channels: u16,
    #[allow(dead_code)]
    pub sample_rate: u32,
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
        let channels = DEFAULT_CHANNELS;
        let sample_rate = DEFAULT_SAMPLE_RATE;

        let host = cpal::default_host();
        // TODO: some way of selecting from available devices?
        let device = host.default_input_device().unwrap();

        // Find supported config for the desired number of channels:
        let mut supported: Option<cpal::SupportedStreamConfigRange> = None;
        for c in device.supported_input_configs().unwrap() {
            if c.channels() == channels {
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
            .with_sample_rate(cpal::SampleRate(sample_rate));

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
                    move |err| {
                        // TODO: should be passed on. Can these be transient?
                        // Should the stream just be closed at this point?
                        println!("Stream error: {}", err);
                    },
                    None, // blocking
                )
                .unwrap(),
        );
        // Apparently *some* platforms don't automatically start the stream
        // so this is possibly necessary.
        stream.play().expect("Failed to start stream");

        InputStream {
            frames: receiver,
            _stream: stream,
        }
    }
}
