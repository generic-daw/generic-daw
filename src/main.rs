use iced::{application, Result};
use iced_fonts::{BOOTSTRAP_FONT_BYTES, REQUIRED_FONT_BYTES};

mod generic_back;

mod generic_front;
use generic_front::Daw;

fn main() -> Result {
    application("GenericDAW", Daw::update, Daw::view)
        .font(REQUIRED_FONT_BYTES)
        .font(BOOTSTRAP_FONT_BYTES)
        .subscription(Daw::subscription)
        .antialiasing(true)
        .run()
}
