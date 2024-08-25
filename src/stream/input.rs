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
