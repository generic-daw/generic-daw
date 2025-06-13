use daw::Daw;
use iced::{Result, daemon};
use icons::LUCIDE_BYTES;

mod arrangement_view;
mod clap_host;
mod components;
mod config;
mod config_view;
mod daw;
mod file_tree;
mod icons;
mod state;
mod stylefns;
mod theme;
mod widget;

fn main() -> Result {
    #[cfg(feature = "env_logger")]
    env_logger::init();

    daemon(Daw::create, Daw::update, Daw::view)
        .title(Daw::title)
        .theme(Daw::theme)
        .subscription(Daw::subscription)
        .antialiasing(true)
        .font(LUCIDE_BYTES)
        .run()
}
