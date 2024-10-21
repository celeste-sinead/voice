use async_channel::Sender;
use iced::{widget, Element, Length, Padding};

use audio::dsp::Decibels;
use audio::stream::executor;
use audio::stream::input::IteratorInput;
use audio::stream::output::OutputDevice;
use audio::stream::pipeline::Identity;
use audio::stream::{ChannelCount, SampleRate};
use audio::synth;

#[derive(Clone, Debug)]
enum Message {
    GainChanged(f32),
}

struct Synthesizer {
    request_sender: Sender<executor::Request>,
    gain: Decibels,
}

impl Default for Synthesizer {
    fn default() -> Synthesizer {
        let channels = ChannelCount::new(1);
        let sample_rate = SampleRate::new(44100);
        let (request_sender, _recv, _join) = executor::PipelineExecutor::new(
            channels,
            sample_rate,
            IteratorInput::new(
                synth::SinIterator::new(sample_rate, 200., 0.),
                sample_rate,
                OutputDevice::DEVICE_BUFFER as usize,
            ),
            Identity::new(),
        );
        Synthesizer {
            request_sender,
            gain: Decibels::new(0.),
        }
    }
}

fn update(synth: &mut Synthesizer, message: Message) {
    match message {
        Message::GainChanged(new_gain) => {
            synth.gain = Decibels::new(new_gain);
            // TODO: can this be async?
            synth
                .request_sender
                .send_blocking(executor::Request::SetGain(synth.gain))
                .unwrap();
        }
    }
}

fn view(synth: &Synthesizer) -> Element<Message> {
    widget::Container::new(widget::column![widget::row![
        widget::text("Gain"),
        widget::Space::new(Length::Fixed(10.), Length::Shrink),
        widget::slider(-100f32..=0f32, f32::from(synth.gain), Message::GainChanged),
        widget::Space::new(Length::Fixed(10.), Length::Shrink),
        widget::text(format!("{}", synth.gain))
    ]])
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding::new(5.))
    .into()
}

fn main() -> iced::Result {
    iced::application("Synthesizer", update, view)
        .antialiasing(true) // see analyzer_app::main
        .run()
}
