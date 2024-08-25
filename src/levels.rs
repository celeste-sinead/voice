use iced::mouse;
use iced::widget::canvas::{Frame, Geometry, Path, Program, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme};

pub struct LevelPlot {}

use super::Message;

impl Program<Message> for LevelPlot {
    type State = ();
    // oh i see this is where you're drawing the cirlce
    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let circle = Path::circle(frame.center(), 50.0);
        frame.fill(&circle, Color::BLACK);
        let rect = Path::rectangle(Point::ORIGIN, Size::new(bounds.width, bounds.height - 1.));
        frame.stroke(&rect, Stroke::default());
        vec![frame.into_geometry()]
    }
}
