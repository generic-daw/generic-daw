mod generic_back;
mod generic_front;

use generic_front::Daw;

fn main() -> iced::Result {
    iced::application("GenericDAW", Daw::update, Daw::view)
        .subscription(Daw::subscription)
        .antialiasing(true)
        .run()
}
