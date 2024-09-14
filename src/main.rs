use std::thread::JoinHandle;

use async_channel;
use async_channel::Receiver;
use iced::executor;
use iced::subscription;
use iced::widget;
use iced::widget::canvas::Canvas;
use iced::{Application, Command, Element, Length, Padding, Settings, Subscription, Theme};

mod levels;
mod mandelbrot;
mod stream;

use levels::LevelPlot;
use stream::executor::{Executor, CHANNEL_MAX};

struct Counter {
    frame: usize, // count frames received by the app
    _audio_thread: JoinHandle<()>,
    audio_messages: Receiver<Message>,
}

// The message type that is used to update iced application state
#[derive(Debug, Clone, Copy)]
enum Message {
    AudioFrame(usize, usize), // TODO: this should be more real, not (frame_number, sample_count)
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
        let (sender, audio_messages) = async_channel::bounded(CHANNEL_MAX);
        let executor = Executor::new(sender);

        (
            Counter {
                frame: 0,
                _audio_thread: executor.start(),
                audio_messages,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Gay gay homosexual gay") // hehe :3
    }

    fn view(&self) -> Element<Message> {
        // Wrap the UI in a Container that can be configured to fill whatever
        // the current window size is, and lay out children to use that space
        widget::Container::new(widget::column![
            widget::text(format!("Frame: {}", self.frame)),
            Canvas::new(LevelPlot {}) // just draws a border and a circle rn
                .width(Length::Fill)
                .height(Length::Fill)
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding::new(5.))
        .into()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::AudioFrame(fr, _len) => self.frame = fr,
            Message::AudioStreamClosed => todo!(),
        };
        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        subscription::unfold(
            SubscriptionId::AudioInput,
            self.audio_messages.clone(),
            |receiver| async {
                let msg = match receiver.recv().await {
                    Ok(m) => m,
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
