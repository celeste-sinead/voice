use async_channel::Receiver;
use iced::executor;
use iced::subscription;
use iced::widget;
use iced::widget::canvas::Canvas;
use iced::{Application, Command, Element, Length, Padding, Settings, Subscription, Theme};

mod levels;
mod stream;

use levels::LevelPlot;
use stream::input::{Frame, InputStream};
use stream::wav::WavWriter;

/*
* hello it is me import antigravity i am going to look through this and try and add comments
* to document this code to see if i understand it yay ^_^
*/

// is usize like the Rust equivalent of the size_t type in C?
struct Counter {
    frame: usize,
    _input: InputStream,
    frame_stream: Receiver<Frame>,
}

// this is for getting buffers from the audio device? i'm guessing the AudioFrame is (n_channels,
// n_samples)?
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

    // this just is the constructor for the application?
    fn new(_flags: ()) -> (Counter, Command<Message>) {
        let input = InputStream::new();
        let writer = WavWriter::new(input.frames.clone());
        let frame_stream = writer.frames.clone();
        writer.run();
        (
            Counter {
                frame: 0,
                _input: input,
                frame_stream,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Gay gay homosexual gay") // hehe :3
    }

    // so is this the method that draws the screen? looks like you're adding that "Frame: " text
    fn view(&self) -> Element<Message> {
        widget::Container::new(widget::column![
            widget::text(format!("Frame: {}", self.frame)),
            Canvas::new(LevelPlot {}) // is this the circle?
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
            Message::AudioStreamClosed => panic!("unexpected"),
        };
        Command::none()
    }

    // i'm gonna need to look into what the fuck a subscription is
    fn subscription(&self) -> Subscription<Message> {
        subscription::unfold(
            SubscriptionId::AudioInput,
            self.frame_stream.clone(),
            |receiver| async {
                let msg = match receiver.recv().await {
                    Ok(f) => Message::AudioFrame(f.number, f.samples.len()),
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
