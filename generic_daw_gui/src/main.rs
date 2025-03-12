use daw::Daw;
use iced::{Result, daemon};

mod arrangement_view;
mod clap_host_view;
mod components;
mod daw;
mod file_tree;
mod icons;
mod stylefns;
mod trace;
mod widget;

fn main() -> Result {
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

    trace::setup();

    daemon(Daw::title, Daw::update, Daw::view)
        .subscription(|_| Daw::subscription())
        .theme(Daw::theme)
        .antialiasing(true)
        .run_with(Daw::create)
}
