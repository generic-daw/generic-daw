mod audio_engine;
mod daw;
mod timeline;
mod track;
mod track_panel;

use daw::Daw;
use iced::{Sandbox, Settings};

fn main() -> iced::Result {
    Daw::run(Settings::default())
}
