use async_channel;
use cpal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct InputStream {
    pub frames: async_channel::Receiver<(usize, Vec<u8>)>,
    _stream: Box<dyn StreamTrait>,
}

impl InputStream {
    pub fn new() -> InputStream {
        let (tx, rx) = async_channel::bounded(16);

        let host = cpal::default_host();
        let device = host.default_input_device().unwrap();
        let mut supported: Option<cpal::SupportedStreamConfigRange> = None;
        for c in device.supported_input_configs().unwrap() {
            if c.channels() == 1 {
                supported = Some(c);
                break;
            }
        }
        // In theory, should check this rate is supported:
        let config = supported.unwrap().with_sample_rate(cpal::SampleRate(44100));
        let mut frame_count: usize = 0;
        let stream = Box::new(
            device
                .build_input_stream(
                    &config.config(),
                    move |data: &[u8], _: &cpal::InputCallbackInfo| {
                        let len = data.len();
                        match tx.try_send((frame_count, Vec::from(data))) {
                            Err(async_channel::TrySendError::Full(_)) => {
                                println!("Dropped {} samples", len);
                            }
                            Err(async_channel::TrySendError::Closed(_)) => {
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
