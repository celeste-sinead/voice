use std::thread::JoinHandle;

use async_channel;
use async_channel::Receiver;
use clap::Parser;
use futures::sink::SinkExt;
use iced::{widget, Element, Length, Padding, Subscription};

mod frequencies;
mod levels;
mod mandelbrot;

use audio::stream::executor::{Executor, CHANNEL_MAX};
use audio::stream::input::{ChannelCount, SampleRate};
use audio::stream::Instant;
use audio::Message;
use frequencies::FrequenciesChart;

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

struct Analyzer {
    time: Instant,
    rms_levels: Vec<f32>,
    _audio_thread: JoinHandle<()>,
    audio_messages: Receiver<Message>,
    frequencies: FrequenciesChart,
}

#[derive(Hash)]
enum SubscriptionId {
    AudioInput,
}

impl Analyzer {
    fn new(args: Args) -> Analyzer {
        let (sender, audio_messages) = async_channel::bounded(CHANNEL_MAX);
        let executor = Executor::new(
            sender,
            ChannelCount::new(args.channels),
            SampleRate::new(args.sample_rate),
        );

        Analyzer {
            time: Instant::default(),
            rms_levels: Vec::new(),
            _audio_thread: executor.start(),
            audio_messages,
            frequencies: FrequenciesChart::new(),
        }
    }
}

fn update(state: &mut Analyzer, message: Message) {
    match message {
        Message::RMSLevels(l) => {
            state.rms_levels = l.values.clone();
            state.time = l.time;
        }
        Message::FFTResult(f) => {
            state.time = f.end_time;
            state.frequencies.update(f);
        }
        Message::AudioStreamClosed => todo!(),
    };
}

fn view(state: &Analyzer) -> Element<Message> {
    // Wrap the UI in a Container that can be configured to fill whatever
    // the current window size is, and lay out children to use that space
    widget::Container::new(widget::column![state.frequencies.view(),])
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding::new(5.))
        .into()
}

fn subscription(state: &Analyzer) -> Subscription<Message> {
    let audio_messages = state.audio_messages.clone();
    Subscription::run_with_id(
        SubscriptionId::AudioInput,
        iced::stream::channel(
            4, // maximum messages waiting in channel
            |mut output| async move {
                loop {
                    match audio_messages.recv().await {
                        Ok(m) => output.send(m).await.unwrap(),
                        Err(_) => {
                            output.send(Message::AudioStreamClosed).await.unwrap();
                            return;
                        }
                    }
                }
            },
        ),
    )
}

fn main() -> iced::Result {
    iced::application("Formant Analyzer", update, view)
        // This is an unreliable work-around for a bug with nvidia's linux
        // vulkan drivers, apparently, see
        // https://github.com/iced-rs/iced/issues/2314
        // If it doesn't work, try setting environment (source env.sh)
        .antialiasing(true)
        .subscription(subscription)
        .run_with(|| (Analyzer::new(Args::parse()), iced::Task::none()))
}
