use std::collections::VecDeque;
use std::iter::zip;
use std::time::Duration;

use iced::{Element, Length};
use plotters_iced::{Chart, ChartBuilder, ChartWidget, DrawingBackend};

use audio::dsp::Decibels;
use audio::{Message, RMSLevels};

pub struct LevelsChart {
    /// The width of the chart
    max_history: Duration,
    /// The time value for each point
    times: VecDeque<Duration>,
    /// By channel, series of levels (dBFS), corresponding to each time
    levels: Vec<VecDeque<Decibels>>,
}

impl Chart<Message> for LevelsChart {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        use plotters::prelude::*;

        let tmin = self.times.front().unwrap_or(&Duration::ZERO).as_secs_f32();
        let tmax = self
            .times
            .back()
            .unwrap_or(&Duration::ZERO)
            .as_secs_f32()
            .max(self.max_history.as_secs_f32());

        let mut chart = builder
            .caption("RMS Levels", ("sans-serif", 20).into_font())
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(40)
            .build_cartesian_2d(tmin..tmax, -50f32..0f32)
            .expect("Failed to build chart");

        chart.configure_mesh().draw().expect("draw mesh");

        // TODO: color list limits to 4 channels (which ought to be enough for anybody?!)
        for (i, (ch, color)) in zip(&self.levels, [BLUE, GREEN, RED, CYAN]).enumerate() {
            chart
                .draw_series(LineSeries::new(
                    zip(&self.times, ch).map(|(t, rms)| (t.as_secs_f32(), f32::from(*rms))),
                    color,
                ))
                .expect("draw series")
                .label(format!("ch{}", i))
                .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color));
        }

        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::LowerLeft)
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()
            .expect("draw series labels");
    }
}

impl LevelsChart {
    pub fn new(max_history: Duration) -> LevelsChart {
        LevelsChart {
            max_history,
            times: VecDeque::new(),
            levels: Vec::new(),
        }
    }

    pub fn view(&self) -> Element<Message> {
        ChartWidget::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn update(&mut self, message: RMSLevels) {
        if self.times.len() == 0 {
            // First update, which tells us the channel count
            self.times.push_back(message.time);
            self.levels.append(
                &mut message
                    .values
                    .into_iter()
                    .map(|l| {
                        let mut v = VecDeque::new();
                        v.push_back(Decibels::from_full_scale(l));
                        v
                    })
                    .collect(),
            );
        } else {
            // Append new point to existing channel buffers
            assert_eq!(self.levels.len(), message.values.len());
            self.times.push_back(message.time);
            for (i, v) in message.values.into_iter().enumerate() {
                self.levels[i].push_back(Decibels::from_full_scale(v));
            }
        }

        // Truncate the beginning of history as it ages out
        if message.time > self.max_history {
            // (avoid underflow..)
            let new_start = message.time - self.max_history;
            while self.times.front().unwrap() < &new_start {
                self.times.pop_front();
                for ch in &mut self.levels {
                    ch.pop_front();
                }
            }
        }
    }
}
