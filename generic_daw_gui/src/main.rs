use daw::Daw;
use iced::{Result, Theme, daemon};

mod arrangement_view;
mod clap_host;
mod components;
mod daw;
mod file_tree;
mod icons;
mod stylefns;
mod widget;

fn main() -> Result {
    env_logger::init();

    daemon(Daw::title, Daw::update, Daw::view)
        .subscription(Daw::subscription)
        .theme(|_, _| Theme::CatppuccinFrappe)
        .antialiasing(true)
        .run_with(Daw::create)
}
