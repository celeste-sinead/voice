use iced::widget;
use iced::{Element, Sandbox, Settings};

struct Counter {
    value: i64,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Increment,
    Decrement,
}

impl Sandbox for Counter {
    type Message = Message;

    fn new() -> Counter {
        Counter { value: 0 }
    }

    fn title(&self) -> String {
        String::from("An example application")
    }

    fn view(&self) -> Element<Message> {
        widget::column![
            widget::button("+").on_press(Message::Increment),
            widget::text(self.value),
            widget::button("-").on_press(Message::Decrement),
        ]
        .into()
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Increment => {
                self.value += 1;
            }
            Message::Decrement => {
                self.value -= 1;
            }
        }
    }
}

fn main() -> iced::Result {
    Counter::run(Settings {
        // This is a work-around for a bug with nvidia's linux vulkan drivers,
        // apparently, see https://github.com/iced-rs/iced/issues/2314
        antialiasing: true,
        ..Settings::default()
    })
}
