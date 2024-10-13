//! Tools for prototyping in a Jupyter notebook
//! The idea is that a notebook can just import this and be unlikely to need
//! anything else, i.e. start with:
//! ```ipynb
//! :dep notebook = { path = "." }
//! use notebook::*;
//! ```

pub use std::f32::consts::PI;

pub use audio;
pub use audio::stream::buffer::{BufferedInput, Period};
pub use audio::stream::input::SampleRate;
pub use audio::{dsp, synth};
pub use num_complex::Complex;
pub use plotters;
use plotters::evcxr::SVGWrapper;
pub use plotters::prelude::*;

pub fn plot_period(period: &Period) -> SVGWrapper {
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

pub fn plot_fft(fft: &dsp::FoldedFFT) -> SVGWrapper {
    evcxr_figure((640, 480), |root| {
        root.fill(&WHITE)?;
        let mut chart = ChartBuilder::on(&root)
            .margin(20)
            .x_label_area_size(40)
            .y_label_area_size(40)
            .right_y_label_area_size(40)
            .build_cartesian_2d(0f32..f32::from(fft.nyquist_frequency()), 0f32..1f32)?
            .set_secondary_coord(0f32..f32::from(fft.nyquist_frequency()), -PI..PI);
        chart
            .configure_mesh()
            .x_max_light_lines(0)
            .y_max_light_lines(0)
            .y_desc("Amplitude (FS)")
            .x_desc("Frequency (Hz)")
            .draw()?;
        chart
            .configure_secondary_axes()
            .y_desc("Phase (radians)")
            .draw()?;

        let magnitudes = fft
            .frequencies()
            .zip(fft.values.iter())
            .map(|(f, (r, _p))| (f32::from(f), *r));
        chart
            .draw_series(LineSeries::new(magnitudes, &RED))
            .unwrap()
            .label("Amplitude")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

        let phases = fft
            .frequencies()
            .zip(fft.values.iter())
            .map(|(f, (_r, p))| (f32::from(f), *p));
        chart
            .draw_secondary_series(LineSeries::new(phases, &BLUE))
            .unwrap()
            .label("Phase")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));

        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperRight)
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()?;

        Ok(())
    })
}
