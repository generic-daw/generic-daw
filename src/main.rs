mod generic_back;
mod generic_front;
mod helpers;

use generic_front::Daw;
use iced::{application, Result};

fn main() -> Result {
    application("GenericDAW", Daw::update, Daw::view)
        .subscription(Daw::subscription)
        .antialiasing(true)
        .run()
}
