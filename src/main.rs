use async_channel;
use cpal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use iced::executor;
use iced::widget;
use iced::{Application, Command, Element, Settings, Theme};

struct Counter {
    value: i64,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Increment,
    Decrement,
}

impl Application for Counter {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;
    type Theme = Theme;

    fn new(_flags: ()) -> (Counter, Command<Message>) {
        (Counter { value: 0 }, Command::none())
    }

    fn title(&self) -> String {
        String::from("An example application")
    }

    fn view(&self) -> Element<Message> {
        widget::column![
            widget::button("+").on_press(Message::Increment),
            widget::text(self.value),
            widget::button("-").on_press(Message::Decrement),
        ]
        .into()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
        };
        Command::none()
    }
}

struct InputStream {
    _frames: async_channel::Receiver<Vec<u8>>,
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
        let stream = Box::new(
            device
                .build_input_stream(
                    &config.config(),
                    move |data: &[u8], _: &cpal::InputCallbackInfo| {
                        let len = data.len();
                        match tx.try_send(Vec::from(data)) {
                            Err(async_channel::TrySendError::Full(_)) => {
                                println!("Dropped {} samples", len);
                            },
                            Err(async_channel::TrySendError::Closed(_)) => {
                                println!("No receiver for {} samples", len);
                            },
                            Ok(()) => {
                                println!("Input: {} samples", data.len());
                            }
                        }
                    },
                    move |err| {
                        println!("Stream error: {}", err);
                    },
                    None, // blocking
                )
                .unwrap(),
        );

        InputStream { _frames: rx, _stream: stream }
    }
}

fn main() -> iced::Result {
    // This will receive data in the background:
    let _stream = InputStream::new();

    Counter::run(Settings {
        // This is a work-around for a bug with nvidia's linux vulkan drivers,
        // apparently, see https://github.com/iced-rs/iced/issues/2314
        antialiasing: true,
        ..Settings::default()
    })
}
