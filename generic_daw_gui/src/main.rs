#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use daw::Daw;
use iced::{Result, daemon};
use icons::LUCIDE_BYTES;
use log::LevelFilter;
#[cfg(unix)]
use wayland_sys as _;

mod action;
mod arrangement_view;
mod clap_host;
mod components;
mod config;
mod config_view;
mod daw;
mod file_tree;
mod icons;
mod lod;
mod state;
mod stylefns;
mod theme;
mod widget;

fn main() -> Result {
	env_logger::builder()
		.filter_module("clap_host", LevelFilter::Warn)
		.filter_module("generic_daw_core", LevelFilter::Warn)
		.filter_module("generic_daw", LevelFilter::Warn)
		.parse_default_env()
		.init();

	daemon(Daw::create, Daw::update, Daw::view)
		.title(Daw::title)
		.theme(Daw::theme)
		.scale_factor(Daw::scale_factor)
		.subscription(Daw::subscription)
		.antialiasing(true)
		.font(LUCIDE_BYTES)
		.run()
}
