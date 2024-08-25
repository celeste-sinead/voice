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

struct Counter {
    frame: usize, // count frames received by the app
    _input: InputStream,
    frame_stream: Receiver<Frame>,
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
        // Open the default audio input
        let input = InputStream::new();
        // Write all the input to session.wav for ad-hoc testing / debugging:
        let writer = WavWriter::new(input.frames.clone());
        // This will be used to subscribe the UI to audio data:
        let frame_stream = writer.frames.clone();
        // Start a thread to write samples out and pass them on:
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
            Message::AudioStreamClosed => panic!("unexpected"),
        };
        Command::none()
    }

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
