use audio::dsp;
use plotters::prelude::*;
use std::f32::consts::PI;

pub fn build_fft_chart<DB: DrawingBackend>(
    mut builder: ChartBuilder<DB>,
    fft: &dsp::FoldedFFT,
) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
    let mut chart = builder
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .right_y_label_area_size(40)
        // TODO: Y axis hackery
        .build_cartesian_2d(0f32..f32::from(fft.nyquist_frequency()), 0f32..0.1f32)?
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

    // TODO: include phase in notebooks, but not UI :/
    // let phases = fft
    //     .frequencies()
    //     .zip(fft.values.iter())
    //     .map(|(f, (_r, p))| (f32::from(f), *p));
    // chart
    //     .draw_secondary_series(LineSeries::new(phases, &BLUE))
    //     .unwrap()
    //     .label("Phase")
    //     .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));

    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    Ok(())
}
