use audio::stream::input::IteratorInput;
use audio::stream::output::OutputDevice;
use audio::stream::pipeline::{Identity, Pipeline};
use audio::stream::{ChannelCount, SampleRate};
use audio::synth;

fn main() {
    let channels = ChannelCount::new(1);
    let sample_rate = SampleRate::new(44100);
    let mut pipeline = Pipeline::new(
        IteratorInput::new(
            synth::sin_iter(sample_rate, 200., 0.),
            sample_rate,
            OutputDevice::DEVICE_BUFFER as usize,
        ),
        Identity::new(),
        OutputDevice::new(channels, sample_rate).unwrap(),
    );
    loop {
        pipeline.process_once().unwrap();
    }
}
