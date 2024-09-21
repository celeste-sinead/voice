use std::ops::Range;

use iced::{Element, Length};
use plotters::prelude::*;
use plotters_iced::{Chart, ChartBuilder, ChartWidget, DrawingBackend};

use crate::Message;

fn draw_mandelbrot<DB: DrawingBackend>(
    mut builder: ChartBuilder<DB>,
) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
    let mut chart = builder
        .margin(20)
        .x_label_area_size(10)
        .y_label_area_size(10)
        .build_cartesian_2d(-2.1f64..0.6f64, -1.2f64..1.2f64)?;

    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_y_mesh()
        .draw()?;

    let plotting_area = chart.plotting_area();

    let range = plotting_area.get_pixel_range();

    let (pw, ph) = (range.0.end - range.0.start, range.1.end - range.1.start);
    let (xr, yr) = (chart.x_range(), chart.y_range());

    for (x, y, c) in mandelbrot_set(xr, yr, (pw as usize, ph as usize), 100) {
        if c != 100 {
            plotting_area.draw_pixel((x, y), &MandelbrotHSL::get_color(c as f64 / 100.0))?;
        } else {
            plotting_area.draw_pixel((x, y), &BLACK)?;
        }
    }

    Ok(())
}

fn mandelbrot_set(
    real: Range<f64>,
    complex: Range<f64>,
    samples: (usize, usize),
    max_iter: usize,
) -> impl Iterator<Item = (f64, f64, usize)> {
    let step = (
        (real.end - real.start) / samples.0 as f64,
        (complex.end - complex.start) / samples.1 as f64,
    );
    (0..(samples.0 * samples.1)).map(move |k| {
        let c = (
            real.start + step.0 * (k % samples.0) as f64,
            complex.start + step.1 * (k / samples.0) as f64,
        );
        let mut z = (0.0, 0.0);
        let mut cnt = 0;
        while cnt < max_iter && z.0 * z.0 + z.1 * z.1 <= 1e10 {
            z = (z.0 * z.0 - z.1 * z.1 + c.0, 2.0 * z.0 * z.1 + c.1);
            cnt += 1;
        }
        (c.0, c.1, cnt)
    })
}

pub struct MandelbrotChart;

impl Chart<Message> for MandelbrotChart {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, builder: ChartBuilder<DB>) {
        draw_mandelbrot(builder).expect("Failed to draw mandelbrot");
    }
}

impl MandelbrotChart {
    #[allow(dead_code)]
    pub fn view(&self) -> Element<Message> {
        ChartWidget::new(self)
            .width(Length::Fixed(400.0))
            .height(Length::Fixed(400.0))
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::path::Path;

    const OUT_FILE_NAME: &str = "plotters-doc-data/mandelbrot.png";

    fn draw_bitmap() -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = Path::new(OUT_FILE_NAME).parent() {
            fs::create_dir_all(parent).expect(&format!("Failed to create dir: {:?}", parent));
        }
        let root = BitMapBackend::new(OUT_FILE_NAME, (800, 600)).into_drawing_area();
        root.fill(&WHITE)?;

        draw_mandelbrot(ChartBuilder::on(&root)).expect("Failed to draw");

        // To avoid the IO failure being ignored silently, we manually call the present function
        root.present().expect("Unable to write result to file, please make sure 'plotters-doc-data' dir exists under current dir");
        println!("Result has been saved to {}", OUT_FILE_NAME);

        Ok(())
    }

    #[test]
    fn entry_point() {
        draw_bitmap().unwrap()
    }
}
