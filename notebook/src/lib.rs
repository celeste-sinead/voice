//! Tools for prototyping in a Jupyter notebook
//! The idea is that a notebook can just import this and be unlikely to need
//! anything else, i.e. start with:
//! ```rust
//! :dep notebook = { path = "." }
//! use notebook::*;
//! ```

pub use audio;
pub use audio::stream::buffer::{BufferedInput, Period};
pub use audio::stream::input::SampleRate;
pub use audio::synth;
pub use plotters;
use plotters::evcxr::SVGWrapper;
use plotters::prelude::*;

pub fn plot_period(period: Period) -> SVGWrapper {
    evcxr_figure((640, 480), |root| {
        assert!(u16::from(period.channel_count()) == 1);

        root.fill(&WHITE)?;
        let mut chart = ChartBuilder::on(&root)
            .margin(20)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(
                f32::from(period.start_time())..f32::from(period.end_time()),
                -1f32..1f32,
            )?;
        chart.configure_mesh().draw()?;

        let series = period
            .get_channel(0)
            .into_timeseries()
            .map(|(t, y)| (f32::from(t), y));

        chart.draw_series(LineSeries::new(series, &RED)).unwrap();

        Ok(())
    })
}