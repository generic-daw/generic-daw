mod generic_back;
mod generic_front;

use generic_front::Daw;
use iced::{Sandbox, Settings};

fn main() -> iced::Result {
    Daw::run(Settings::default())
}
