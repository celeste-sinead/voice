use std::thread::JoinHandle;
use std::time::Duration;

use async_channel;
use async_channel::Receiver;
use clap::Parser;
use iced::executor;
use iced::subscription;
use iced::widget;
use iced::{Application, Command, Element, Length, Padding, Settings, Subscription, Theme};

mod levels;
mod mandelbrot;

use audio::stream::executor::{Executor, CHANNEL_MAX};
use audio::stream::input::{ChannelCount, SampleRate};
use audio::Message;
use levels::LevelsChart;

struct Counter {
    time: Duration,
    rms_levels: Vec<f32>,
    _audio_thread: JoinHandle<()>,
    audio_messages: Receiver<Message>,
    levels: LevelsChart,
}

#[derive(Hash)]
enum SubscriptionId {
    AudioInput,
}

#[derive(Debug, Parser)]
struct Args {
    /// The number of channels for audio input
    #[arg(short, long, default_value_t = 2)]
    channels: u16,
    /// The sample rate (Hz) for audio input
    #[arg(short, long, default_value_t = 44100)]
    sample_rate: u32,
}

impl Default for Args {
    fn default() -> Args {
        Args {
            channels: 2,
            sample_rate: 44100,
        }
    }
}

impl Application for Counter {
    type Executor = executor::Default;
    type Flags = Args;
    type Message = Message;
    type Theme = Theme;

    fn new(args: Args) -> (Counter, Command<Message>) {
        let (sender, audio_messages) = async_channel::bounded(CHANNEL_MAX);
        let executor = Executor::new(
            sender,
            ChannelCount::new(args.channels),
            SampleRate::new(args.sample_rate),
        );

        (
            Counter {
                time: Duration::default(),
                rms_levels: Vec::new(),
                _audio_thread: executor.start(),
                audio_messages,
                levels: LevelsChart::new(Duration::from_secs(30)),
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
        widget::Container::new(widget::column![self.levels.view(),])
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding::new(5.))
            .into()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::RMSLevels(l) => {
                self.rms_levels = l.values.clone();
                self.time = l.time;
                self.levels.update(l);
            }
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
    let args = Args::parse();
    Counter::run(Settings {
        flags: args,
        // This is an unreliable work-around for a bug with nvidia's linux
        // vulkan drivers, apparently, see
        // https://github.com/iced-rs/iced/issues/2314
        // If it doesn't work, try setting environment (source env.sh)
        antialiasing: true,
        ..Settings::default()
    })
}
