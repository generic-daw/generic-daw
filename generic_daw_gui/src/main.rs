use daw::Daw;
use iced::{Result, application};
use iced_fonts::{BOOTSTRAP_FONT_BYTES, REQUIRED_FONT_BYTES};

pub(crate) mod arrangement_view;
pub(crate) mod clap_host_view;
pub(crate) mod daw;
mod trace;
pub(crate) mod widget;

fn main() -> Result {
    trace::setup();

    #[cfg(target_os = "linux")]
    {
        // SAFETY:
        // the program is single-threaded at this point
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") }

        if std::env::var("WINIT_X11_SCALE_FACTOR").is_err() {
            // SAFETY:
            // the program is single-threaded at this point
            unsafe { std::env::set_var("WINIT_X11_SCALE_FACTOR", "1.0") }
        }
    }

    application("GenericDAW", Daw::update, Daw::view)
        .font(REQUIRED_FONT_BYTES)
        .font(BOOTSTRAP_FONT_BYTES)
        .subscription(Daw::subscription)
        .theme(Daw::theme)
        .antialiasing(true)
        .run()
}
