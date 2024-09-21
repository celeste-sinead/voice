use std::thread::JoinHandle;
use std::time::Duration;

use async_channel;
use async_channel::Receiver;
use clap::Parser;
use iced::executor;
use iced::subscription;
use iced::widget;
use iced::{Application, Command, Element, Length, Padding, Settings, Subscription, Theme};
use plotters_iced::{Chart, ChartBuilder, ChartWidget, DrawingBackend};

mod dsp;
mod mandelbrot;
mod stream;

use stream::executor::{Executor, CHANNEL_MAX};
use stream::input::{ChannelCount, SampleRate};

struct Counter {
    time: Duration,
    rms_levels: Vec<f32>,
    _audio_thread: JoinHandle<()>,
    audio_messages: Receiver<Message>,
    chart: ExampleChart,
}

// The message type that is used to update iced application state
#[derive(Debug, Clone)]
enum Message {
    RMSLevels { time: Duration, values: Vec<f32> },
    AudioStreamClosed,
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
                chart: ExampleChart,
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
            widget::text(format!(
                "Time: {}:{:01}, Levels: {:?}",
                self.time.as_secs(),
                self.time.subsec_millis() / 100,
                self.rms_levels
            )),
            self.chart.view(),
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding::new(5.))
        .into()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::RMSLevels { time, values } => {
                self.rms_levels = values;
                self.time = time;
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

struct ExampleChart;

impl Chart<Message> for ExampleChart {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        use plotters::prelude::*;
        let mut chart = builder
            .caption("y=x^2", ("sans-serif", 20).into_font())
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(-1f32..1f32, -0.1f32..1f32)
            .expect("Failed to build chart");

        chart.configure_mesh().draw().expect("draw mesh");

        chart
            .draw_series(LineSeries::new(
                (-50..=50).map(|x| x as f32 / 50.0).map(|x| (x, x * x)),
                &RED,
            ))
            .expect("draw series")
            .label("y = x^2")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

        chart
            .configure_series_labels()
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()
            .expect("draw series labels");
    }
}

impl ExampleChart {
    fn view(&self) -> Element<Message> {
        ChartWidget::new(self)
            .width(Length::Fixed(400.0))
            .height(Length::Fixed(400.0))
            .into()
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
