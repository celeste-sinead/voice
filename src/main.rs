use iced::executor;
use iced::subscription;
use iced::widget;
use iced::{Application, Command, Element, Settings, Subscription, Theme};

mod stream;
use stream::InputStream;

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
