use async_channel::Sender;
use iced::{widget, Element, Length, Padding};

use audio::dsp::Decibels;
use audio::stream::buffer::FrameAccumulator;
use audio::stream::executor;
use audio::stream::output::OutputDevice;
use audio::stream::pipeline::{Chain, Pipeline};
use audio::stream::{ChannelCount, SampleRate};
use audio::synth::{Gain, SinIterator};

#[derive(Clone, Debug)]
enum Message {
    FrequencyChanged(f32),
    GainChanged(f32),
}

struct Synthesizer {
    request_sender: Sender<Message>,
    gain: Decibels,
    frequency: f32,
}

impl Default for Synthesizer {
    fn default() -> Synthesizer {
        let channels = ChannelCount::new(1);
        let sample_rate = SampleRate::new(44100);
        let (request_sender, _recv, _join) = executor::PipelineExecutor::new(
            channels,
            sample_rate,
            SinIterator::new(sample_rate, 200., 0.),
            Chain::new(
                Gain::new(Decibels::new(0.)),
                FrameAccumulator::new(channels, sample_rate, OutputDevice::DEVICE_BUFFER as usize),
            ),
            Box::new(update_pipeline),
        );
        Synthesizer {
            request_sender,
            gain: Decibels::new(0.),
            frequency: 200.,
        }
    }
}

fn update(synth: &mut Synthesizer, message: Message) {
    match message {
        Message::GainChanged(new_gain) => {
            synth.gain = Decibels::new(new_gain);
            // TODO: can this be async?
            synth.request_sender.send_blocking(message).unwrap();
        }
        Message::FrequencyChanged(new_freq) => {
            synth.frequency = new_freq;
            synth.request_sender.send_blocking(message).unwrap();
        }
    }
}

fn view(synth: &Synthesizer) -> Element<Message> {
    widget::Container::new(widget::column![
        widget::row![
            widget::text("Gain"),
            widget::Space::new(Length::Fixed(10.), Length::Shrink),
            widget::slider(-40f32..=0f32, f32::from(synth.gain), Message::GainChanged),
            widget::Space::new(Length::Fixed(10.), Length::Shrink),
            widget::text(format!("{}", synth.gain))
        ],
        widget::row![
            widget::text("Pitch"),
            widget::Space::new(Length::Fixed(10.), Length::Shrink),
            widget::slider(
                50f32..=2000f32,
                f32::from(synth.frequency),
                Message::FrequencyChanged
            ),
            widget::Space::new(Length::Fixed(10.), Length::Shrink),
            widget::text(format!("{} Hz", synth.frequency))
        ]
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding::new(5.))
    .into()
}

fn update_pipeline(
    p: &mut Pipeline<SinIterator, Chain<Gain, FrameAccumulator>, OutputDevice>,
    cmd: Message,
) {
    match cmd {
        Message::GainChanged(gain) => p.step_mut().first_mut().set_gain(Decibels::new(gain)),
        Message::FrequencyChanged(freq) => p.input_mut().set_frequency(freq),
    }
}

fn main() -> iced::Result {
    iced::application("Synthesizer", update, view)
        .antialiasing(true) // see analyzer_app::main
        .run()
}
