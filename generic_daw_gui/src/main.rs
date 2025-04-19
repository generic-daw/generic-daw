use daw::Daw;
use iced::{Result, Theme, daemon};
use icons::LUCIDE;

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

    daemon(Daw::create, Daw::update, Daw::view)
        .title(Daw::title)
        .subscription(Daw::subscription)
        .theme(|_, _| Theme::CatppuccinFrappe)
        .antialiasing(true)
        .font(LUCIDE)
        .run()
}
