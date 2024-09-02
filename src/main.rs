mod generic_back;
mod generic_front;

use generic_front::Daw;
use iced::{Application, Settings};

fn main() -> iced::Result {
    Daw::run(Settings::<()> {
        antialiasing: true,
        ..Default::default()
    })
}
