use iced::{Element, Length};
use plotters_iced::{Chart, ChartBuilder, ChartWidget, DrawingBackend};

use audio::{FFTResult, Message};
use charts;

pub struct FrequenciesChart {
    latest_ffts: Option<FFTResult>,
}

impl FrequenciesChart {
    pub fn new() -> FrequenciesChart {
        FrequenciesChart { latest_ffts: None }
    }

    pub fn view(&self) -> Element<Message> {
        ChartWidget::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn update(&mut self, message: FFTResult) {
        self.latest_ffts = Some(message);
    }
}

impl Chart<Message> for FrequenciesChart {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, builder: ChartBuilder<DB>) {
        if let Some(latest) = self.latest_ffts.as_ref() {
            // TODO: display more than the first channel (and don't show phases)
            charts::build_fft_chart(builder, latest.ffts.first().unwrap())
                .expect("Failed to build chart");
        }
    }
}
