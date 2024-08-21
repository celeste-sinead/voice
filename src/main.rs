use async_channel;
use cpal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use iced::executor;
use iced::subscription;
use iced::widget;
use iced::{Application, Command, Element, Settings, Subscription, Theme};

struct Counter {
    frame: usize,
    stream: InputStream,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    AudioFrame(usize, usize),
    AudioStreamClosed,
}

#[derive(Hash)]
enum SubscriptionId {
    AudioInput,
}

impl Application for Counter {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;
    type Theme = Theme;

    fn new(_flags: ()) -> (Counter, Command<Message>) {
        (
            Counter {
                frame: 0,
                stream: InputStream::new(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("An example application")
    }

    fn view(&self) -> Element<Message> {
        widget::column![widget::text(format!("Frame: {}", self.frame)),].into()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::AudioFrame(fr, _len) => self.frame = fr,
            Message::AudioStreamClosed => panic!("unexpected"),
        };
        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        subscription::unfold(
            SubscriptionId::AudioInput,
            self.stream.frames.clone(),
            |receiver| async {
                let msg = match receiver.recv().await {
                    Ok((fr, samples)) => Message::AudioFrame(fr, samples.len()),
                    Err(_) => Message::AudioStreamClosed,
                };
                (msg, receiver)
            },
        )
    }
}

struct InputStream {
    frames: async_channel::Receiver<(usize, Vec<u8>)>,
    _stream: Box<dyn StreamTrait>,
}

impl InputStream {
    fn new() -> InputStream {
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

fn main() -> iced::Result {
    Counter::run(Settings {
        // This is an unreliable work-around for a bug with nvidia's linux
        // vulkan drivers, apparently, see
        // https://github.com/iced-rs/iced/issues/2314
        // If it doesn't work, try setting environment (source env.sh)
        antialiasing: true,
        ..Settings::default()
    })
}
