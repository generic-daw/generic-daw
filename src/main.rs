use generic_front::Daw;
use iced::{application, Result};
use iced_fonts::{BOOTSTRAP_FONT_BYTES, REQUIRED_FONT_BYTES};

mod clap_host;
mod generic_back;
mod generic_front;

fn main() -> Result {
    #[cfg(target_os = "linux")]
    unsafe {
        if std::env::var("WINIT_X11_SCALE_FACTOR").is_err() {
            std::env::set_var("WINIT_X11_SCALE_FACTOR", "1.0");
        }
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    application("GenericDAW", Daw::update, Daw::view)
        .font(REQUIRED_FONT_BYTES)
        .font(BOOTSTRAP_FONT_BYTES)
        .subscription(Daw::subscription)
        .theme(Daw::theme)
        .antialiasing(true)
        .run()
}
